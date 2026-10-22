use std::{
    fs::OpenOptions,
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
};

use crate::{
    loader::{Reader, RUNS_FILE},
    syntax::{parse as parse_file, quote, Field, Value},
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

    pub fn parse(text: &str) -> Option<Outcome> {
        match text {
            "pass" => Some(Outcome::Pass),
            "fail" => Some(Outcome::Fail),
            "error" => Some(Outcome::Error),
            "skip" => Some(Outcome::Skip),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub example: String,
    /// The example's digest when it ran.
    pub digest: String,
    pub digest_version: u64,
    pub channel: String,
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
        let fields = block.fields_by_name(path, &FIELDS, "run", "run")?;
        let identifier = |name: &str| match fields.required(name)? {
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
        let outcome_field = fields.required("outcome")?;
        let outcome = outcome_field
            .value
            .as_ident()
            .and_then(Outcome::parse)
            .ok_or_else(|| {
                format!(
                    "{path}:{}: unknown outcome {}; expected pass, fail, error or skip",
                    outcome_field.line, outcome_field.value
                )
            })?;
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
        let version = fields.required("digest_version")?;
        let Value::Int(digest_version) = version.value else {
            return Err(format!(
                "{path}:{}: digest_version must be an integer",
                version.line
            ));
        };
        runs.push(Run {
            example: identifier("example")?,
            digest: string(fields.required("digest")?)?,
            digest_version,
            channel: identifier("channel")?,
            source: string(fields.required("source")?)?,
            base: string(fields.required("base")?)?,
            outcome,
            diff,
        });
    }
    Ok(runs)
}

/// The catalog's own ledger, [`RUNS_FILE`]; a missing ledger holds no runs.
pub fn read(reader: &dyn Reader) -> Result<Vec<Run>, String> {
    parse(&reader.read_or_empty(RUNS_FILE)?, RUNS_FILE)
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
