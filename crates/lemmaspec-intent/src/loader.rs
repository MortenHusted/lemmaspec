use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use crate::{
    syntax::{parse, Block, Field, File, Value},
    Catalog, ExceptionKind, ItemKind, Statement, StatementKind,
};

pub const ROOT_FILE: &str = "catalog.intent";
pub const APPROVALS_FILE: &str = "approvals.intent";
pub const RUNS_FILE: &str = "runs.intent";

/// Files of one catalog directory, addressed by `/`-separated relative paths.
pub trait Reader {
    fn read(&self, path: &str) -> io::Result<String>;
    /// Every file under the catalog directory.
    fn list(&self) -> io::Result<Vec<String>>;

    /// A missing file reads as empty; any other error reads `path: error`.
    fn read_or_empty(&self, path: &str) -> Result<String, String> {
        match self.read(path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(String::new()),
            read => read.map_err(|e| format!("{path}: {e}")),
        }
    }
}

impl Reader for BTreeMap<String, String> {
    fn read(&self, path: &str) -> io::Result<String> {
        self.get(path)
            .cloned()
            .ok_or_else(|| io::ErrorKind::NotFound.into())
    }

    fn list(&self) -> io::Result<Vec<String>> {
        Ok(self.keys().cloned().collect())
    }
}

pub struct DirReader(pub PathBuf);

impl Reader for DirReader {
    fn read(&self, path: &str) -> io::Result<String> {
        fs::read_to_string(self.0.join(path))
    }

    fn list(&self) -> io::Result<Vec<String>> {
        let mut files = Vec::new();
        list_dir(&self.0, "", &mut files)?;
        Ok(files)
    }
}

fn list_dir(dir: &Path, prefix: &str, files: &mut Vec<String>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = format!("{prefix}{}", entry.file_name().to_string_lossy());
        let kind = entry.file_type()?;
        if kind.is_dir() {
            list_dir(&entry.path(), &format!("{path}/"), files)?;
        } else if kind.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

struct Loaded {
    file: File,
    target_by_alias: BTreeMap<String, String>,
}

/// Load the catalog rooted at [`ROOT_FILE`] and everything it imports, then
/// check IDs, references and link kinds. Fails on the first error.
pub fn load(reader: &dyn Reader) -> Result<Catalog, String> {
    let source = reader.read(ROOT_FILE).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => format!("{ROOT_FILE} not found in the catalog directory"),
        _ => format!("{ROOT_FILE}: {e}"),
    })?;
    let mut files = vec![loaded(parse(&source, ROOT_FILE)?)?];
    walk_imports(reader, 0, &mut files, &mut vec![ROOT_FILE.into()])?;
    let mut paths = reader
        .list()
        .map_err(|e| format!("cannot list the catalog directory: {e}"))?;
    paths.sort();
    for path in paths {
        if path.ends_with(".intent")
            && path != APPROVALS_FILE
            && path != RUNS_FILE
            && !files.iter().any(|loaded| loaded.file.path == path)
        {
            return Err(format!("{path}: not imported from {ROOT_FILE}"));
        }
    }
    statements(&files)
}

fn loaded(file: File) -> Result<Loaded, String> {
    if file.catalog.is_none() {
        return Err(format!(
            "{}:1: a catalog file must start with catalog NAME",
            file.path
        ));
    }
    Ok(Loaded {
        file,
        target_by_alias: BTreeMap::new(),
    })
}

fn walk_imports(
    reader: &dyn Reader,
    index: usize,
    files: &mut Vec<Loaded>,
    chain: &mut Vec<String>,
) -> Result<(), String> {
    let importer = files[index].file.path.clone();
    for import in files[index].file.imports.clone() {
        let at = format!("{importer}:{}", import.line);
        let target = import_target(&importer, &import.path).ok_or_else(|| {
            format!(
                "{at}: import {:?} must be a relative path inside the catalog directory",
                import.path
            )
        })?;
        if chain.contains(&target) {
            return Err(format!("{at}: import cycle through {target}"));
        }
        if files[index]
            .target_by_alias
            .insert(import.alias.clone(), target.clone())
            .is_some()
        {
            return Err(format!("{at}: duplicate import alias {}", import.alias));
        }
        if files.iter().any(|loaded| loaded.file.path == target) {
            continue;
        }
        let source = reader.read(&target).map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => format!("{at}: imported file {target} not found"),
            _ => format!("{target}: {e}"),
        })?;
        files.push(loaded(parse(&source, &target)?)?);
        chain.push(target);
        walk_imports(reader, files.len() - 1, files, chain)?;
        chain.pop();
    }
    Ok(())
}

fn import_target(importer: &str, target: &str) -> Option<String> {
    let target = target.replace('\\', "/");
    if target.starts_with('/') || target.contains(':') {
        return None;
    }
    let mut parts: Vec<&str> = importer.split('/').collect();
    parts.pop();
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    Some(parts.join("/"))
}

struct Declaration<'a> {
    kind: ItemKind,
    file: &'a str,
    line: usize,
}

fn statements(files: &[Loaded]) -> Result<Catalog, String> {
    let mut declared = BTreeMap::new();
    for loaded in files {
        let path = loaded.file.path.as_str();
        for block in &loaded.file.blocks {
            let at = format!("{path}:{}", block.line);
            let Some(kind) = ItemKind::parse(&block.kind).filter(|kind| *kind != ItemKind::Example)
            else {
                return Err(format!("{at}: unknown statement kind {}", block.kind));
            };
            let id = block
                .name
                .as_deref()
                .ok_or_else(|| format!("{at}: {} needs an id", block.kind))?;
            let declaration = Declaration {
                kind,
                file: path,
                line: block.line,
            };
            if let Some(other) = declared.insert(id, declaration) {
                return Err(format!(
                    "duplicate id {id} in {}:{} and {at}",
                    other.file, other.line
                ));
            }
        }
    }
    let mut statements = BTreeMap::new();
    for loaded in files {
        for block in &loaded.file.blocks {
            let statement = statement(block, loaded, &declared)?;
            statements.insert(statement.id.clone(), statement);
        }
    }
    Ok(Catalog { statements })
}

fn statement(
    block: &Block,
    loaded: &Loaded,
    declared: &BTreeMap<&str, Declaration>,
) -> Result<Statement, String> {
    let path = &loaded.file.path;
    let id = block.name.as_deref().unwrap_or_default();
    let declaration = &declared[id];
    let allowed: &[&str] = match declaration.kind {
        ItemKind::Definition => &["text", "refines"],
        ItemKind::Exception => &["text", "narrows", "kind", "channels"],
        _ => &["text"],
    };
    let subject = format!("{} {id}", block.kind);
    let fields = block.fields_by_name(path, allowed, &subject, id)?;
    let link = |field: &Field, value: &Value, kind: ItemKind| -> Result<String, String> {
        let at = format!("{path}:{}", field.line);
        let (target, local) = match value {
            Value::Ident(target) => (target, Some(path.as_str())),
            Value::Qualified(alias, target) => (
                target,
                loaded.target_by_alias.get(alias).map(String::as_str),
            ),
            _ => {
                return Err(format!(
                    "{at}: {} of {id} must be a statement reference",
                    field.name
                ))
            }
        };
        match declared.get(target.as_str()) {
            Some(found) if Some(found.file) == local => {
                if found.kind != kind {
                    return Err(format!(
                        "{at}: {} of {id} must name {}, but {value} is {}",
                        field.name,
                        article(kind),
                        article(found.kind)
                    ));
                }
                Ok(target.clone())
            }
            _ => Err(format!(
                "{at}: unknown reference {value} in {} of {id}",
                field.name
            )),
        }
    };
    let text = fields.required("text")?;
    let text = match &text.value {
        Value::Str(prose) if !prose.trim().is_empty() => prose.clone(),
        _ => {
            return Err(format!(
                "{path}:{}: text of {id} must be a nonempty string",
                text.line
            ))
        }
    };
    let kind = match declaration.kind {
        ItemKind::Intent => StatementKind::Intent,
        ItemKind::Channel => StatementKind::Channel,
        ItemKind::Definition => {
            let refines = fields.required("refines")?;
            StatementKind::Definition {
                refines: link(refines, &refines.value, ItemKind::Intent)?,
            }
        }
        ItemKind::Exception => {
            let narrows = fields.required("narrows")?;
            let narrows = link(narrows, &narrows.value, ItemKind::Definition)?;
            let kind_field = fields.required("kind")?;
            let kind = kind_field
                .value
                .as_ident()
                .and_then(ExceptionKind::parse)
                .ok_or_else(|| {
                    format!(
                        "{path}:{}: exception {id} has unknown kind {}; expected does_not_apply or holds_instead",
                        kind_field.line, kind_field.value
                    )
                })?;
            let field = fields.required("channels")?;
            let at = format!("{path}:{}", field.line);
            let Value::List(items) = &field.value else {
                return Err(format!("{at}: channels of {id} must be a list"));
            };
            if items.is_empty() {
                return Err(format!("{at}: exception {id} names no channels"));
            }
            let mut channels = Vec::new();
            for item in items {
                let channel = link(field, item, ItemKind::Channel)?;
                if channels.contains(&channel) {
                    return Err(format!(
                        "{at}: exception {id} names channel {channel} twice"
                    ));
                }
                channels.push(channel);
            }
            channels.sort();
            StatementKind::Exception {
                narrows,
                kind,
                channels,
            }
        }
        ItemKind::Example => unreachable!("statements() declares no example"),
    };
    Ok(Statement {
        id: id.into(),
        text,
        file: path.clone(),
        kind,
    })
}

fn article(kind: ItemKind) -> String {
    let kind = kind.as_str();
    let article = if kind.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    };
    format!("{article} {kind}")
}
