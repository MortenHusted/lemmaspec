use std::{collections::BTreeMap, fmt, fs::OpenOptions, io::Write, path::Path};

pub use crate::ItemKind;
use crate::{
    digest::{statement_digest, DIGEST_VERSION},
    examples::{scan, Example, Scan},
    loader::{load, DirReader, Reader, APPROVALS_FILE},
    syntax::{parse, quote, Block, Field, Value},
    Catalog, Statement, StatementKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Approve,
    Remove,
}

impl Action {
    pub fn as_str(self) -> &'static str {
        match self {
            Action::Approve => "approve",
            Action::Remove => "remove",
        }
    }

    pub fn parse(text: &str) -> Option<Action> {
        match text {
            "approve" => Some(Action::Approve),
            "remove" => Some(Action::Remove),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub item: String,
    pub kind: ItemKind,
    pub action: Action,
    pub by: String,
    /// The item's digest when approved; a removal repeats its last approved digest.
    pub digest: String,
    pub digest_version: u64,
    /// Each resolved link's target with that target's digest at this entry: a
    /// definition's refines, an exception's narrows, an example's statement.
    pub links: BTreeMap<String, String>,
    /// An example's channel tags, sorted.
    pub channels: Vec<String>,
    pub text: Option<String>,
}

impl fmt::Display for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "approval {{")?;
        writeln!(f, "  item: {}", self.item)?;
        writeln!(f, "  kind: {}", self.kind.as_str())?;
        writeln!(f, "  action: {}", self.action.as_str())?;
        writeln!(f, "  by: {}", quote(&self.by))?;
        writeln!(f, "  digest: {}", quote(&self.digest))?;
        writeln!(f, "  digest_version: {}", self.digest_version)?;
        if !self.links.is_empty() {
            let links: Vec<String> = self
                .links
                .iter()
                .map(|(target, digest)| format!("{target}: {}", quote(digest)))
                .collect();
            writeln!(f, "  links: [{}]", links.join(", "))?;
        }
        if !self.channels.is_empty() {
            writeln!(f, "  channels: [{}]", self.channels.join(", "))?;
        }
        if let Some(text) = &self.text {
            writeln!(f, "  text: {}", quote(text))?;
        }
        writeln!(f, "}}")
    }
}

/// The approval log's entries in file order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Log {
    pub entries: Vec<Entry>,
}

impl Log {
    /// Parse and validate the log's entries. Errors read `approvals.intent:<line>: ...`.
    pub fn parse(source: &str) -> Result<Log, String> {
        let file = parse(source, APPROVALS_FILE)?;
        if let Some(name) = file.catalog {
            return Err(format!(
                "{APPROVALS_FILE}:1: expected approval entries, found catalog {name}"
            ));
        }
        let entries = file.blocks.iter().map(entry).collect::<Result<_, _>>()?;
        Ok(Log { entries })
    }

    /// Read [`APPROVALS_FILE`] from the catalog directory; a missing log is empty.
    pub fn read(reader: &dyn Reader) -> Result<Log, String> {
        Log::parse(&reader.read_or_empty(APPROVALS_FILE)?)
    }

    /// The last entry for each item, whatever its action.
    pub fn latest(&self) -> BTreeMap<&str, &Entry> {
        self.entries
            .iter()
            .map(|entry| (entry.item.as_str(), entry))
            .collect()
    }

    /// The latest entries of approved statements and examples that are
    /// missing now, in item order. They still count until a removal is
    /// approved (R15). An example counts as present only when the scan
    /// accepted it.
    pub fn reinstated(&self, catalog: &Catalog, examples: &Scan) -> Vec<&Entry> {
        self.latest()
            .into_values()
            .filter(|entry| entry.action == Action::Approve)
            .filter(|entry| {
                !catalog.statements.contains_key(&entry.item)
                    && !examples.examples.iter().any(|e| e.id == entry.item)
            })
            .collect()
    }
}

const FIELDS: [&str; 9] = [
    "item",
    "kind",
    "action",
    "by",
    "digest",
    "digest_version",
    "links",
    "channels",
    "text",
];

fn entry(block: &Block) -> Result<Entry, String> {
    let at = |line: usize| format!("{APPROVALS_FILE}:{line}");
    if block.kind != "approval" || block.name.is_some() {
        let found = match &block.name {
            Some(name) => format!("{} {name}", block.kind),
            None => block.kind.clone(),
        };
        return Err(format!(
            "{}: expected an approval entry, found {found}",
            at(block.line)
        ));
    }
    let item = match block.fields.iter().find(|field| field.name == "item") {
        None => return Err(format!("{}: approval needs item", at(block.line))),
        Some(Field {
            value: Value::Ident(id),
            ..
        }) => id.clone(),
        Some(field) => {
            return Err(format!(
                "{}: item of an approval must be an item ID",
                at(field.line)
            ))
        }
    };
    let subject = format!("approval of {item}");
    let fields = block.fields_by_name(APPROVALS_FILE, &FIELDS, &subject, &subject)?;
    let string = |field: &Field| match &field.value {
        Value::Str(text) => Ok(text.clone()),
        _ => Err(format!(
            "{}: {} of {item} must be a string",
            at(field.line),
            field.name
        )),
    };

    let field = fields.required("kind")?;
    let kind = field
        .value
        .as_ident()
        .and_then(ItemKind::parse)
        .ok_or_else(|| {
            format!(
                "{}: approval of {item} has unknown kind {}; expected intent, definition, exception, channel or example",
                at(field.line),
                field.value
            )
        })?;
    let field = fields.required("action")?;
    let action = field
        .value
        .as_ident()
        .and_then(Action::parse)
        .ok_or_else(|| {
            format!(
                "{}: approval of {item} has unknown action {}; expected approve or remove",
                at(field.line),
                field.value
            )
        })?;
    let by = string(fields.required("by")?)?;
    let digest = string(fields.required("digest")?)?;
    let field = fields.required("digest_version")?;
    let Value::Int(digest_version) = field.value else {
        return Err(format!(
            "{}: digest_version of {item} must be an integer",
            at(field.line)
        ));
    };
    let mut links = BTreeMap::new();
    if let Some(field) = fields.get("links") {
        let malformed = || {
            format!(
                "{}: links of {item} must be a list of target: \"digest\" entries",
                at(field.line)
            )
        };
        let Value::List(values) = &field.value else {
            return Err(malformed());
        };
        for value in values {
            let Value::Entry(target, digest) = value else {
                return Err(malformed());
            };
            let Value::Str(digest) = digest.as_ref() else {
                return Err(malformed());
            };
            if links.insert(target.clone(), digest.clone()).is_some() {
                return Err(format!(
                    "{}: links of {item} name {target} twice",
                    at(field.line)
                ));
            }
        }
    }
    let mut channels = Vec::new();
    if let Some(field) = fields.get("channels") {
        let malformed = || {
            format!(
                "{}: channels of {item} must be a list of channel IDs",
                at(field.line)
            )
        };
        let Value::List(values) = &field.value else {
            return Err(malformed());
        };
        for value in values {
            let Value::Ident(channel) = value else {
                return Err(malformed());
            };
            channels.push(channel.clone());
        }
    }
    let text = fields.get("text").map(string).transpose()?;
    Ok(Entry {
        item,
        kind,
        action,
        by,
        digest,
        digest_version,
        links,
        channels,
        text,
    })
}

enum Item<'a> {
    Statement(&'a Statement),
    Example(&'a Example),
}

fn find<'a>(
    catalog: &'a Catalog,
    examples: &'a Scan,
    id: &str,
) -> Result<Option<Item<'a>>, String> {
    let statement = catalog.statements.get(id);
    let example = examples.examples.iter().find(|example| example.id == id);
    match (statement, example) {
        (Some(_), Some(_)) => Err(format!("{id} names both a statement and an example")),
        (Some(statement), None) => Ok(Some(Item::Statement(statement))),
        (None, Some(example)) => Ok(Some(Item::Example(example))),
        (None, None) => Ok(None),
    }
}

fn current<'a>(catalog: &'a Catalog, examples: &'a Scan, id: &str) -> Result<Item<'a>, String> {
    find(catalog, examples, id)?.ok_or_else(|| format!("unknown item {id}"))
}

fn approval(item: &Item, catalog: &Catalog, by: &str) -> Result<Entry, String> {
    match item {
        Item::Statement(statement) => {
            let link = match &statement.kind {
                StatementKind::Definition { refines } => Some(refines.as_str()),
                StatementKind::Exception { narrows, .. } => Some(narrows.as_str()),
                StatementKind::Intent | StatementKind::Channel => None,
            };
            Ok(Entry {
                item: statement.id.clone(),
                kind: ItemKind::of(&statement.kind),
                action: Action::Approve,
                by: by.into(),
                digest: statement_digest(statement),
                digest_version: DIGEST_VERSION,
                links: current_links(catalog, &statement.id, link)?,
                channels: Vec::new(),
                text: Some(statement.text.clone()),
            })
        }
        Item::Example(example) => Ok(Entry {
            item: example.id.clone(),
            kind: ItemKind::Example,
            action: Action::Approve,
            by: by.into(),
            digest: example.digest.clone(),
            digest_version: DIGEST_VERSION,
            links: current_links(catalog, &example.id, Some(&example.statement))?,
            channels: example.channels.clone(),
            text: None,
        }),
    }
}

fn current_links(
    catalog: &Catalog,
    id: &str,
    target: Option<&str>,
) -> Result<BTreeMap<String, String>, String> {
    let mut links = BTreeMap::new();
    if let Some(target) = target {
        let statement = catalog
            .statements
            .get(target)
            .ok_or_else(|| format!("{id} links to {target}, which is not in the catalog"))?;
        links.insert(target.to_string(), statement_digest(statement));
    }
    Ok(links)
}

fn approver(by: &str) -> Result<(), String> {
    if by.trim().is_empty() {
        return Err("an approval needs the approver's name".into());
    }
    if by.chars().any(char::is_control) {
        return Err(format!(
            "the approver's name {by:?} has a control character"
        ));
    }
    Ok(())
}

/// The entries approving each named statement or example as it reads now,
/// with the current digest of each item it links to.
pub fn approve(
    catalog: &Catalog,
    examples: &Scan,
    ids: &[impl AsRef<str>],
    by: &str,
) -> Result<String, String> {
    approver(by)?;
    let mut text = String::new();
    for id in ids {
        let item = current(catalog, examples, id.as_ref())?;
        text += &approval(&item, catalog, by)?.to_string();
    }
    Ok(text)
}

/// Like [`approve`], for items whose own digest is unchanged since their
/// latest entry approved them, so only the linked digests can differ (R14).
pub fn reconfirm(
    log: &Log,
    catalog: &Catalog,
    examples: &Scan,
    ids: &[impl AsRef<str>],
    by: &str,
) -> Result<String, String> {
    approver(by)?;
    let latest = log.latest();
    let mut text = String::new();
    for id in ids {
        let id = id.as_ref();
        let entry = approval(&current(catalog, examples, id)?, catalog, by)?;
        let Some(approved) = latest.get(id).filter(|e| e.action == Action::Approve) else {
            return Err(format!("{id} is not approved; approve it instead"));
        };
        if (&approved.digest, approved.digest_version) != (&entry.digest, entry.digest_version) {
            return Err(format!(
                "{id} changed since its latest approval; approve it instead"
            ));
        }
        text += &entry.to_string();
    }
    Ok(text)
}

/// The entries removing each named approved item, refused while an item that
/// is approved, and not removed by this same call, recorded a link to it.
pub fn remove(
    log: &Log,
    catalog: &Catalog,
    examples: &Scan,
    ids: &[impl AsRef<str>],
    by: &str,
) -> Result<String, String> {
    approver(by)?;
    let latest = log.latest();
    let removing: Vec<&str> = ids.iter().map(AsRef::as_ref).collect();
    let mut text = String::new();
    for &id in &removing {
        let present = find(catalog, examples, id)?.is_some();
        let approved = match latest.get(id) {
            Some(entry) if entry.action == Action::Approve => *entry,
            None if !present => return Err(format!("unknown item {id}")),
            _ => {
                return Err(format!(
                    "{id} is not approved, so there is nothing to remove"
                ))
            }
        };
        let dependents: Vec<&str> = latest
            .values()
            .filter(|entry| entry.action == Action::Approve && entry.links.contains_key(id))
            .map(|entry| entry.item.as_str())
            .filter(|item| !removing.contains(item))
            .collect();
        if !dependents.is_empty() {
            return Err(format!(
                "cannot remove {id}: approved items still link to it: {}",
                dependents.join(", ")
            ));
        }
        text += &Entry {
            action: Action::Remove,
            by: by.into(),
            links: BTreeMap::new(),
            channels: Vec::new(),
            text: None,
            ..approved.clone()
        }
        .to_string();
    }
    Ok(text)
}

/// Load the catalog in `dir` and the examples in `tests`, then append the
/// entries `change` returns to the log in `dir`. Nothing is written when the
/// catalog, the examples, the log or the change fails, and the log's existing
/// bytes always stay an unchanged prefix.
pub fn append(
    dir: &Path,
    tests: &Path,
    change: impl FnOnce(&Log, &Catalog, &Scan) -> Result<String, String>,
) -> Result<(), String> {
    let reader = DirReader(dir.into());
    let catalog = load(&reader)?;
    let examples = scan(tests, &catalog)?;
    let source = reader.read_or_empty(APPROVALS_FILE)?;
    let mut text = change(&Log::parse(&source)?, &catalog, &examples)?;
    if !source.is_empty() && !source.ends_with('\n') {
        text.insert(0, '\n');
    }
    let path = dir.join(APPROVALS_FILE);
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut file| file.write_all(text.as_bytes()))
        .map_err(|e| format!("{}: {e}", path.display()))
}
