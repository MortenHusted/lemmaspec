//! A small project in a temporary git repository, with a catalog in
//! `intent/`, tagged examples in `tests/` and a source file in `src/`.

#![allow(dead_code)]

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use lemmaspec_intent::{
    approvals::{append, approve},
    digest::DIGEST_VERSION,
    examples::scan,
    identity::Repo,
    loader::{load, DirReader, RUNS_FILE},
    report::{report, Options, Report},
    runs::{self, Outcome, Run},
};

pub const CHANNELS: &str = r#"catalog channels {
  channel cli {
    text: "The lemmaspec command line."
  }
  channel library {
    text: "The lemmaspec Rust library."
  }
}
"#;

/// The statements of the root catalog, which imports the channels as `ch`.
pub const STATEMENTS: &str = r#"  intent results_are_honest {
    text: "LemmaSpec never reports an artifact as passing while one of its claims is open."
  }
  definition walk_exit_status {
    refines: results_are_honest
    text: "walk exits 0 when every expectation holds, 1 when one is open, and 2 when the artifact is invalid."
  }
  exception library_returns_report {
    narrows: walk_exit_status
    channels: [ch.library]
    kind: holds_instead
    text: "The library returns the report value, and its status field carries the same outcomes."
  }
"#;

pub const EXAMPLES: &str = r#"/// intent: example open_exits_one exemplifies walk_exit_status on cli
#[test]
fn open_expectation_exits_one() {}

/// intent: example library_report exemplifies library_returns_report on library
#[test]
fn library_returns_the_report() {}
"#;

/// Every statement and example of the project.
pub const ITEMS: [&str; 7] = [
    "cli",
    "library",
    "results_are_honest",
    "walk_exit_status",
    "library_returns_report",
    "open_exits_one",
    "library_report",
];

/// `path` as the report cites it, with forward slashes on every platform.
pub fn cited(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub fn root_catalog(body: &str) -> String {
    format!("catalog fixture {{\n  use \"./channels.intent\" as ch\n\n{body}}}\n")
}

pub struct Project {
    pub dir: PathBuf,
}

impl Project {
    pub fn new(label: &str) -> Project {
        let dir = std::env::temp_dir().join(format!(
            "lemmaspec-intent-report-{label}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let project = Project { dir };
        project.git(&["init", "-q"]);
        for (key, value) in [
            ("user.email", "intent@example.com"),
            ("user.name", "Intent Test"),
            ("commit.gpgsign", "false"),
            ("core.autocrlf", "false"),
            ("core.hooksPath", "no-hooks"),
        ] {
            project.git(&["config", key, value]);
        }
        project.write(".gitignore", "/ci/\n");
        project.write("src/lib.rs", "pub fn open_status() -> u8 {\n    1\n}\n");
        project.write("intent/catalog.intent", &root_catalog(STATEMENTS));
        project.write("intent/channels.intent", CHANNELS);
        project.write("tests/walk.rs", EXAMPLES);
        project.commit("initial");
        project
    }

    pub fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.dir)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }

    pub fn write(&self, path: &str, content: &str) {
        let path = self.dir.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    pub fn read(&self, path: &str) -> String {
        fs::read_to_string(self.dir.join(path)).unwrap()
    }

    /// Replace the first occurrence of `from`, which must be there.
    pub fn edit(&self, path: &str, from: &str, to: &str) {
        let text = self.read(path);
        assert!(text.contains(from), "{path} has no {from:?}");
        self.write(path, &text.replacen(from, to, 1));
    }

    pub fn commit(&self, message: &str) -> String {
        self.git(&["add", "-A"]);
        self.git(&[
            "commit",
            "-q",
            "--no-verify",
            "--allow-empty",
            "-m",
            message,
        ]);
        self.git(&["rev-parse", "HEAD"]).trim().to_string()
    }

    pub fn intent(&self) -> PathBuf {
        self.dir.join("intent")
    }

    pub fn tests(&self) -> PathBuf {
        self.dir.join("tests")
    }

    pub fn repo(&self) -> Repo {
        Repo::of_catalog(&self.intent()).unwrap()
    }

    pub fn identity(&self) -> String {
        self.repo().working_tree_identity().unwrap()
    }

    pub fn approve(&self, ids: &[&str], by: &str) {
        append(&self.intent(), &self.tests(), |_, catalog, examples| {
            approve(catalog, examples, ids, by)
        })
        .unwrap();
    }

    pub fn approve_all(&self) {
        self.approve(&ITEMS, "owner");
    }

    /// Append one run of every example on each of its channels to `ledger`.
    pub fn record(&self, ledger: &Path, outcome: Outcome, source: &str, diff: &str) {
        let catalog = load(&DirReader(self.intent())).unwrap();
        let examples = scan(&self.tests(), &catalog).unwrap().examples;
        let runs: Vec<Run> = examples
            .iter()
            .flat_map(|example| {
                example.channels.iter().map(|channel| Run {
                    example: example.id.clone(),
                    digest: example.digest.clone(),
                    digest_version: DIGEST_VERSION,
                    channel: channel.clone(),
                    source: source.into(),
                    base: "0000000".into(),
                    outcome,
                    diff: (outcome == Outcome::Fail).then(|| diff.into()),
                })
            })
            .collect();
        runs::append(ledger, &runs).unwrap();
    }

    /// Every example failed on a scratch break, in the committed ledger.
    pub fn record_red(&self) {
        self.record(
            &self.intent().join(RUNS_FILE),
            Outcome::Fail,
            "sha256:scratch",
            "a scratch break",
        );
    }

    /// Every example passes at the current identity, in an ignored ledger
    /// under `ci/`, as CI records them.
    pub fn record_green(&self, name: &str) -> PathBuf {
        let path = self.dir.join("ci").join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        self.record(&path, Outcome::Pass, &self.identity(), "");
        path
    }

    pub fn report(&self, ledgers: &[PathBuf], base: Option<&str>) -> Report {
        report(&Options {
            dir: self.intent(),
            tests: self.tests(),
            ledgers: ledgers.to_vec(),
            base: base.map(Into::into),
        })
        .unwrap()
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}
