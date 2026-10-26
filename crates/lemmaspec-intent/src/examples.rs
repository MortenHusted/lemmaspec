use std::{collections::BTreeMap, fs, io, path::Path};

use syn::{Attribute, Expr, ExprLit, Item, ItemFn, ItemMod, Lit, Meta};

use crate::{digest::example_digest, Catalog, ExceptionKind, StatementKind};

const TAG: &str = "intent: example <id> exemplifies <statement> on <channel>[, <channel>...]";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Example {
    pub id: String,
    pub statement: String,
    /// Sorted.
    pub channels: Vec<String>,
    /// The test file's stem, as `cargo test --test` takes it.
    pub file: String,
    /// `module::function` inside the file, as `cargo test -- --exact` takes it.
    pub test: String,
    /// The tag's line in `<file>.rs`.
    pub line: usize,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExampleIssue {
    /// `None` when the tag is too malformed to name one.
    pub example: Option<String>,
    pub file: String,
    pub test: String,
    pub line: usize,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scan {
    pub examples: Vec<Example>,
    pub issues: Vec<ExampleIssue>,
}

struct Found {
    file: String,
    test: String,
    line: usize,
    tag: String,
    digest: String,
}

/// What one file holds: its tagged tests, and how many `#[test]` fns, tagged
/// or not, have each `module::function` path.
#[derive(Default)]
struct Collected {
    tagged: Vec<Found>,
    paths: BTreeMap<String, usize>,
}

/// Scan the `*.rs` files directly inside `dir`, in file name order. A missing
/// `dir` has no examples. Tag problems become issues; an unreadable file, a
/// parse error or two tags claiming one example ID fail, with locations given
/// as `<file>.rs:<line>`.
pub fn scan(dir: &Path, catalog: &Catalog) -> Result<Scan, String> {
    let unreadable = |e| format!("cannot read {}: {e}", dir.display());
    let entries = match fs::read_dir(dir) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Scan::default()),
        entries => entries.map_err(unreadable)?,
    };
    let mut paths = Vec::new();
    for entry in entries {
        let path = entry.map_err(unreadable)?.path();
        if path.is_file() && path.extension().is_some_and(|extension| extension == "rs") {
            paths.push(path);
        }
    }
    paths.sort();
    let mut files = Vec::new();
    for path in paths {
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let source = fs::read_to_string(&path)
            .map_err(|e| format!("{stem}.rs: {e}"))?
            .replace("\r\n", "\n");
        let file = syn::parse_file(&source)
            .map_err(|e| format!("{stem}.rs:{}: {e}", e.span().start().line))?;
        let mut collected = Collected::default();
        let context: Vec<&Attribute> = file.attrs.iter().collect();
        collect(&file.items, &stem, "", &context, &mut collected);
        files.push(collected);
    }
    let mut scan = Scan::default();
    let mut claimed = BTreeMap::new();
    for (found, paths) in files.into_iter().flat_map(|file| {
        file.tagged
            .into_iter()
            .map(move |found| (found, file.paths.clone()))
    }) {
        let location = format!("{}.rs:{}", found.file, found.line);
        let issue = |example: Option<&str>, reason: String| ExampleIssue {
            example: example.map(Into::into),
            file: found.file.clone(),
            test: found.test.clone(),
            line: found.line,
            reason,
        };
        let Some((id, statement, channels)) = parse_tag(&found.tag) else {
            let reason = format!("malformed tag {:?}; expected {TAG}", found.tag);
            scan.issues.push(issue(None, reason));
            continue;
        };
        if let Some(other) = claimed.insert(id.clone(), location.clone()) {
            return Err(format!(
                "duplicate example id {id} in {other} and {location}"
            ));
        }
        if paths[&found.test] > 1 {
            let reason = format!(
                "another test in {}.rs has the path {}, so cargo test --exact cannot tell them apart",
                found.file, found.test
            );
            scan.issues.push(issue(Some(&id), reason));
            continue;
        }
        if let Some(reason) = cannot_exemplify(catalog, &statement, &channels) {
            scan.issues.push(issue(Some(&id), reason));
            continue;
        }
        scan.examples.push(Example {
            id,
            statement,
            channels,
            file: found.file,
            test: found.test,
            line: found.line,
            digest: found.digest,
        });
    }
    Ok(scan)
}

/// `context` holds the attributes of the file and of every module around
/// `items`, outermost first: a `#[cfg]` there decides whether a test is
/// compiled at all, so it counts toward each example's digest.
fn collect(
    items: &[Item],
    file: &str,
    module: &str,
    context: &[&Attribute],
    collected: &mut Collected,
) {
    for item in items {
        match item {
            Item::Fn(function) if is_test(function) => {
                let test = format!("{module}{}", function.sig.ident);
                *collected.paths.entry(test.clone()).or_default() += 1;
                let tags: Vec<_> = doc_lines(&function.attrs)
                    .into_iter()
                    .filter(|(_, text)| text.starts_with("intent:"))
                    .collect();
                if tags.is_empty() {
                    continue;
                }
                let digest = example_digest(context, function);
                for (line, tag) in tags {
                    collected.tagged.push(Found {
                        file: file.into(),
                        test: test.clone(),
                        line,
                        tag,
                        digest: digest.clone(),
                    });
                }
            }
            Item::Mod(ItemMod {
                attrs,
                ident,
                content: Some((_, items)),
                ..
            }) => {
                let context: Vec<&Attribute> = context.iter().copied().chain(attrs).collect();
                collect(
                    items,
                    file,
                    &format!("{module}{ident}::"),
                    &context,
                    collected,
                );
            }
            _ => {}
        }
    }
}

fn is_test(function: &ItemFn) -> bool {
    function
        .attrs
        .iter()
        .any(|attribute| attribute.path().is_ident("test"))
}

fn doc_lines(attributes: &[Attribute]) -> Vec<(usize, String)> {
    let mut lines = Vec::new();
    for attribute in attributes {
        let Meta::NameValue(meta) = &attribute.meta else {
            continue;
        };
        let Expr::Lit(ExprLit {
            lit: Lit::Str(text),
            ..
        }) = &meta.value
        else {
            continue;
        };
        if meta.path.is_ident("doc") {
            let first = attribute.pound_token.spans[0].start().line;
            for (offset, line) in text.value().lines().enumerate() {
                lines.push((first + offset, line.trim().to_string()));
            }
        }
    }
    lines
}

fn parse_tag(tag: &str) -> Option<(String, String, Vec<String>)> {
    let words: Vec<&str> = tag.split_whitespace().collect();
    let ["intent:", "example", id, "exemplifies", statement, "on", channels @ ..] =
        words.as_slice()
    else {
        return None;
    };
    let mut channels: Vec<String> = channels
        .join(" ")
        .split(',')
        .map(|channel| channel.trim().to_string())
        .collect();
    channels.sort();
    let distinct = channels.windows(2).all(|pair| pair[0] != pair[1]);
    let named = [id, statement]
        .into_iter()
        .copied()
        .chain(channels.iter().map(String::as_str))
        .all(is_identifier);
    (distinct && named).then(|| (id.to_string(), statement.to_string(), channels))
}

fn is_identifier(word: &str) -> bool {
    word.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && word.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn cannot_exemplify(catalog: &Catalog, statement: &str, channels: &[String]) -> Option<String> {
    let kind = |id: &str| catalog.statements.get(id).map(|found| &found.kind);
    let scope = match kind(statement) {
        None => return Some(format!("unknown statement {statement}")),
        Some(StatementKind::Definition { .. }) => None,
        Some(StatementKind::Exception {
            kind: ExceptionKind::HoldsInstead,
            channels,
            ..
        }) => Some(channels),
        Some(other) => {
            return Some(format!(
            "{statement} is {}; an example exemplifies a definition or a holds_instead exception",
            described(other)
        ))
        }
    };
    channels.iter().find_map(|channel| match kind(channel) {
        None => Some(format!("unknown channel {channel}")),
        Some(StatementKind::Channel) => {
            let scope = scope.filter(|scope| !scope.contains(channel))?;
            Some(format!(
                "{statement} applies only on {}, not on {channel}",
                scope.join(", ")
            ))
        }
        Some(other) => Some(format!("{channel} is {}, not a channel", described(other))),
    })
}

fn described(kind: &StatementKind) -> String {
    match kind {
        StatementKind::Intent => "an intent".into(),
        StatementKind::Definition { .. } => "a definition".into(),
        StatementKind::Exception { kind, .. } => format!("a {} exception", kind.as_str()),
        StatementKind::Channel => "a channel".into(),
    }
}
