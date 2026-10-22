//! Finds tagged examples among a project's `cargo test` integration tests.
//!
//! An example is a `#[test]` function whose doc comment has the line
//! `intent: example <id> exemplifies <statement> on <channel>, <channel>`.

use std::{collections::BTreeMap, fs, path::Path};

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

/// A tag that does not make its test an example, and why.
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

/// A tag line found on a `#[test]` function.
struct Found {
    file: String,
    test: String,
    line: usize,
    tag: String,
    digest: String,
}

/// Scan the `*.rs` files directly inside `dir`, in file name order. Tag
/// problems become issues; an unreadable file, a parse error or two tags
/// claiming one example ID fail, with locations given as `<file>.rs:<line>`.
pub fn scan(dir: &Path, catalog: &Catalog) -> Result<Scan, String> {
    let unreadable = |e| format!("cannot read {}: {e}", dir.display());
    let mut paths = Vec::new();
    for entry in fs::read_dir(dir).map_err(unreadable)? {
        let path = entry.map_err(unreadable)?.path();
        if path.is_file() && path.extension().is_some_and(|extension| extension == "rs") {
            paths.push(path);
        }
    }
    paths.sort();
    let mut found = Vec::new();
    for path in paths {
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let source = fs::read_to_string(&path)
            .map_err(|e| format!("{stem}.rs: {e}"))?
            .replace("\r\n", "\n");
        let file = syn::parse_file(&source)
            .map_err(|e| format!("{stem}.rs:{}: {e}", e.span().start().line))?;
        collect(&file.items, &stem, "", &mut found);
    }
    let mut scan = Scan::default();
    let mut claimed = BTreeMap::new();
    for found in found {
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
        if let Some(reason) = cannot_exemplify(catalog, &statement) {
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

fn collect(items: &[Item], file: &str, module: &str, found: &mut Vec<Found>) {
    for item in items {
        match item {
            Item::Fn(function) if is_test(function) => {
                let tags: Vec<_> = doc_lines(&function.attrs)
                    .into_iter()
                    .filter(|(_, text)| text.starts_with("intent:"))
                    .collect();
                if tags.is_empty() {
                    continue;
                }
                let digest = example_digest(function);
                for (line, tag) in tags {
                    found.push(Found {
                        file: file.into(),
                        test: format!("{module}{}", function.sig.ident),
                        line,
                        tag,
                        digest: digest.clone(),
                    });
                }
            }
            Item::Mod(ItemMod {
                ident,
                content: Some((_, items)),
                ..
            }) => collect(items, file, &format!("{module}{ident}::"), found),
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

/// Each trimmed doc comment line with its line number.
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

fn cannot_exemplify(catalog: &Catalog, statement: &str) -> Option<String> {
    let kind = match catalog.statements.get(statement).map(|found| &found.kind) {
        None => return Some(format!("unknown statement {statement}")),
        Some(
            StatementKind::Definition { .. }
            | StatementKind::Exception {
                kind: ExceptionKind::HoldsInstead,
                ..
            },
        ) => return None,
        Some(StatementKind::Intent) => "an intent",
        Some(StatementKind::Channel) => "a channel",
        Some(StatementKind::Exception { .. }) => "a does_not_apply exception",
    };
    Some(format!(
        "{statement} is {kind}; an example exemplifies a definition or a holds_instead exception"
    ))
}
