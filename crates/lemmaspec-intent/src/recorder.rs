//! Runs tagged examples through `cargo test` (KTD8) and appends one run per
//! example and channel to a run ledger. This is the only place that runs
//! project code (KTD11).

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::{
    examples::Example,
    identity::Repo,
    runs::{self, Outcome, Run},
};

/// The environment variable that tells an example which channel it runs on.
pub const CHANNEL_VAR: &str = "LEMMASPEC_INTENT_CHANNEL";

/// Whether a test run exited successfully, and what it printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutput {
    pub success: bool,
    /// Standard output followed by standard error.
    pub output: String,
}

/// Runs one test of one integration test file on one channel.
pub trait Runner {
    /// `file` is the test file's stem and `test` its `module::function` path.
    fn run(&mut self, file: &str, test: &str, channel: &str) -> Result<RunOutput, String>;
}

/// Runs examples with `cargo test` in a package directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoTest {
    pub dir: PathBuf,
}

impl CargoTest {
    /// `cargo test --locked --test <file> -- --exact <test>` with the channel set.
    pub fn command(&self, file: &str, test: &str, channel: &str) -> Command {
        let mut command = Command::new("cargo");
        command
            .args(["test", "--locked", "--test", file, "--", "--exact", test])
            .env(CHANNEL_VAR, channel)
            .current_dir(&self.dir);
        command
    }
}

impl Runner for CargoTest {
    fn run(&mut self, file: &str, test: &str, channel: &str) -> Result<RunOutput, String> {
        let output = self
            .command(file, test, channel)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("cannot run cargo test in {}: {e}", self.dir.display()))?;
        Ok(RunOutput {
            success: output.status.success(),
            output: format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        })
    }
}

/// The source a recording runs against.
pub trait Source {
    /// The source identity of the code the examples run on.
    fn identity(&self) -> Result<String, String>;
    /// The commit the source is based on.
    fn base(&self) -> Result<String, String>;
    /// The tracked-file diff against `base`, stored with failing runs.
    fn diff_against(&self, base: &str) -> Result<String, String>;
}

impl Source for Repo {
    fn identity(&self) -> Result<String, String> {
        self.working_tree_identity()
    }

    fn base(&self) -> Result<String, String> {
        self.head()
    }

    fn diff_against(&self, base: &str) -> Result<String, String> {
        self.diff(base)
    }
}

/// Which examples to run, and on which of their channels. The default
/// selects every example on every channel it is tagged for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Selection {
    pub example: Option<String>,
    pub channel: Option<String>,
}

/// Run every selected example once per selected channel it is tagged for and
/// append one run each to `ledger`. Each run stores the example's digest from
/// before the run. Nothing is appended when a run cannot start or when the
/// source identity changes while the examples run.
pub fn record(
    examples: &[Example],
    selection: &Selection,
    runner: &mut dyn Runner,
    source: &dyn Source,
    ledger: &Path,
) -> Result<Vec<Run>, String> {
    let selected = select(examples, selection)?;
    if selected.is_empty() {
        return Ok(Vec::new());
    }
    let identity = source.identity()?;
    let base = source.base()?;
    let mut failure_diff = None;
    let mut recorded = Vec::new();
    for (example, channel) in selected {
        let outcome = classify(&runner.run(&example.file, &example.test, channel)?);
        let diff = if outcome == Outcome::Fail {
            if failure_diff.is_none() {
                failure_diff = Some(source.diff_against(&base)?);
            }
            failure_diff.clone()
        } else {
            None
        };
        recorded.push(Run {
            example: example.id.clone(),
            digest: example.digest.clone(),
            digest_version: crate::digest::DIGEST_VERSION,
            channel: channel.clone(),
            source: identity.clone(),
            base: base.clone(),
            outcome,
            diff,
        });
    }
    let after = source.identity()?;
    if after != identity {
        return Err(format!(
            "the source changed while recording, from {identity} to {after}; no runs were recorded"
        ));
    }
    runs::append(ledger, &recorded)?;
    Ok(recorded)
}

fn select<'a>(
    examples: &'a [Example],
    selection: &Selection,
) -> Result<Vec<(&'a Example, &'a String)>, String> {
    let chosen: Vec<&Example> = match &selection.example {
        Some(id) => vec![examples
            .iter()
            .find(|example| &example.id == id)
            .ok_or_else(|| format!("unknown example {id}"))?],
        None => examples.iter().collect(),
    };
    let selected: Vec<_> = chosen
        .into_iter()
        .flat_map(|example| {
            example
                .channels
                .iter()
                .map(move |channel| (example, channel))
        })
        .filter(|(_, channel)| {
            selection
                .channel
                .as_ref()
                .is_none_or(|only| only == *channel)
        })
        .collect();
    match (&selection.example, &selection.channel) {
        (Some(id), Some(channel)) if selected.is_empty() => {
            Err(format!("example {id} is not tagged for channel {channel}"))
        }
        (None, Some(channel)) if selected.is_empty() => {
            Err(format!("no example is tagged for channel {channel}"))
        }
        _ => Ok(selected),
    }
}

/// Pass, fail or skip when libtest's last summary line counts exactly one
/// test that passed, failed or was ignored and the exit status agrees;
/// otherwise error, which covers a build failure (no summary line) and a
/// path matching no test.
fn classify(ran: &RunOutput) -> Outcome {
    let Some(summary) = ran
        .output
        .lines()
        .rev()
        .find_map(|line| line.trim().strip_prefix("test result: "))
    else {
        return Outcome::Error;
    };
    let counts = summary.split_once(". ").map_or("", |(_, counts)| counts);
    let count = |label: &str| {
        counts.split(';').find_map(|part| {
            let (number, name) = part.trim().split_once(' ')?;
            (name == label).then(|| number.parse::<u64>().ok())?
        })
    };
    match (
        ran.success,
        count("passed"),
        count("failed"),
        count("ignored"),
    ) {
        (true, Some(1), Some(0), Some(0)) => Outcome::Pass,
        (false, Some(0), Some(1), Some(0)) => Outcome::Fail,
        (true, Some(0), Some(0), Some(1)) => Outcome::Skip,
        _ => Outcome::Error,
    }
}
