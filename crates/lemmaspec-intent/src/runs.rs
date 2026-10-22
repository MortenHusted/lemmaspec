//! The run ledger: `run { ... }` blocks in `.intent` syntax, read by fixed
//! path and only ever appended to.

use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
};

use crate::{
    loader::{Reader, RUNS_FILE},
    syntax::{parse as parse_file, Field, Value},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Fail,
    Error,
    Skip,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Pass => "pass",
            Outcome::Fail => "fail",
            Outcome::Error => "error",
            Outcome::Skip => "skip",
        }
    }
}

/// One recorded run of an example on a channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub example: String,
    /// The example's digest when it ran.
    pub digest: String,
    pub digest_version: u64,
    pub channel: String,
    /// The source identity the example ran against.
    pub source: String,
    /// The commit HEAD pointed to, abbreviated or full.
    pub base: String,
    pub outcome: Outcome,
    /// For a failing run only: the tracked-file diff against `base`, without
    /// the run ledger and the approval log.
    pub diff: Option<String>,
}

const FIELDS: [&str; 8] = [
    "example",
    "digest",
    "digest_version",
    "channel",
    "source",
    "base",
    "outcome",
    "diff",
];

/// Parse and validate a ledger. `path` names it in errors as `path:line: message`.
pub fn parse(source: &str, path: &str) -> Result<Vec<Run>, String> {
    let file = parse_file(source, path)?;
    let mut runs = Vec::new();
    for block in &file.blocks {
        let at = format!("{path}:{}", block.line);
        if block.kind != "run" {
            return Err(format!(
                "{at}: unknown block {}; the run ledger holds only run blocks",
                block.kind
            ));
        }
        if block.name.is_some() {
            return Err(format!("{at}: a run block takes no name"));
        }
        let mut fields = BTreeMap::new();
        for field in &block.fields {
            let at = format!("{path}:{}", field.line);
            if !FIELDS.contains(&field.name.as_str()) {
                return Err(format!("{at}: run has no field {}", field.name));
            }
            if fields.insert(field.name.as_str(), field).is_some() {
                return Err(format!("{at}: duplicate field {} in run", field.name));
            }
        }
        let required = |name: &str| {
            fields
                .get(name)
                .copied()
                .ok_or_else(|| format!("{at}: run needs {name}"))
        };
        let identifier = |name: &str| match required(name)? {
            Field {
                value: Value::Ident(value),
                ..
            } => Ok(value.clone()),
            field => Err(format!(
                "{path}:{}: {name} must be an identifier",
                field.line
            )),
        };
        let string = |field: &Field| match &field.value {
            Value::Str(value) => Ok(value.clone()),
            _ => Err(format!(
                "{path}:{}: {} must be a string",
                field.line, field.name
            )),
        };
        let outcome_field = required("outcome")?;
        let outcome = match &outcome_field.value {
            Value::Ident(name) if name == "pass" => Outcome::Pass,
            Value::Ident(name) if name == "fail" => Outcome::Fail,
            Value::Ident(name) if name == "error" => Outcome::Error,
            Value::Ident(name) if name == "skip" => Outcome::Skip,
            other => {
                return Err(format!(
                    "{path}:{}: unknown outcome {other}; expected pass, fail, error or skip",
                    outcome_field.line
                ))
            }
        };
        let diff = match (fields.get("diff"), outcome) {
            (Some(field), Outcome::Fail) => Some(string(field)?),
            (None, Outcome::Fail) => return Err(format!("{at}: a failing run needs diff")),
            (Some(field), _) => {
                return Err(format!(
                    "{path}:{}: only a failing run stores a diff",
                    field.line
                ))
            }
            (None, _) => None,
        };
        let version = required("digest_version")?;
        let Value::Int(digest_version) = version.value else {
            return Err(format!(
                "{path}:{}: digest_version must be an integer",
                version.line
            ));
        };
        runs.push(Run {
            example: identifier("example")?,
            digest: string(required("digest")?)?,
            digest_version,
            channel: identifier("channel")?,
            source: string(required("source")?)?,
            base: string(required("base")?)?,
            outcome,
            diff,
        });
    }
    Ok(runs)
}

/// The catalog's own ledger, [`RUNS_FILE`]; a missing ledger holds no runs.
pub fn read(reader: &dyn Reader) -> Result<Vec<Run>, String> {
    match reader.read(RUNS_FILE) {
        Ok(source) => parse(&source, RUNS_FILE),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(format!("{RUNS_FILE}: {e}")),
    }
}

/// Append `runs` to the ledger at `path`, creating it if needed. The existing
/// bytes are never rewritten, so they stay a prefix of the new ledger.
pub fn append(path: &Path, runs: &[Run]) -> Result<(), String> {
    let failed = |e: io::Error| format!("cannot append to {}: {e}", path.display());
    let mut file = OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .open(path)
        .map_err(failed)?;
    let mut text = String::new();
    if file.metadata().map_err(failed)?.len() > 0 {
        let mut last = [0];
        file.seek(SeekFrom::End(-1)).map_err(failed)?;
        file.read_exact(&mut last).map_err(failed)?;
        if last != *b"\n" {
            text.push('\n');
        }
    }
    for run in runs {
        text.push_str(&render(run));
    }
    file.write_all(text.as_bytes()).map_err(failed)
}

fn render(run: &Run) -> String {
    let mut text = format!(
        "run {{\n  example: {}\n  digest: {}\n  digest_version: {}\n  channel: {}\n  source: {}\n  base: {}\n  outcome: {}\n",
        run.example,
        quote(&run.digest),
        run.digest_version,
        run.channel,
        quote(&run.source),
        quote(&run.base),
        run.outcome.as_str(),
    );
    if let Some(diff) = &run.diff {
        text.push_str(&format!("  diff: {}\n", quote(diff)));
    }
    text.push_str("}\n");
    text
}

/// A string literal using only the escapes the `.intent` lexer reads.
fn quote(text: &str) -> String {
    let mut quoted = String::from('"');
    for c in text.chars() {
        match c {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}
