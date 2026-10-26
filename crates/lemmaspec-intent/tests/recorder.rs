use std::{
    cell::Cell,
    collections::BTreeMap,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use lemmaspec_intent::{
    digest::DIGEST_VERSION,
    examples::Example,
    identity::Repo,
    loader::{DirReader, RUNS_FILE},
    recorder::{record, CargoTest, RunOutput, Runner, Selection, Source},
    runs::{self, Outcome, Run},
};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
const SOURCE: &str = "sha256:5c4a7c2b";
const BASE: &str = "3bf8c7d9e0a1b2c3d4e5f60718293a4b5c6d7e8f";

/// Real `cargo test --locked --test <stem> -- --exact <path>` output, stdout
/// then stderr, captured from a probe crate.
fn canned(name: &str) -> RunOutput {
    RunOutput {
        success: matches!(name, "pass" | "ignored" | "no_match"),
        output: fs::read_to_string(format!("{FIXTURES}/runs/{name}.txt")).unwrap(),
    }
}

/// Answers each test path with canned output and remembers every call.
struct FakeRunner {
    outputs: BTreeMap<String, RunOutput>,
    calls: Vec<(String, String, String)>,
}

impl FakeRunner {
    fn answering(outputs: &[(&str, RunOutput)]) -> FakeRunner {
        FakeRunner {
            outputs: outputs
                .iter()
                .map(|(test, output)| (test.to_string(), output.clone()))
                .collect(),
            calls: Vec::new(),
        }
    }
}

impl Runner for FakeRunner {
    fn run(&mut self, file: &str, test: &str, channel: &str) -> Result<RunOutput, String> {
        self.calls.push((file.into(), test.into(), channel.into()));
        self.outputs
            .get(test)
            .cloned()
            .ok_or_else(|| format!("no canned output for {test}"))
    }
}

/// Answers successive identity calls from `identities`, repeating the last.
struct FakeSource {
    identities: Vec<&'static str>,
    calls: Cell<usize>,
}

fn fake_source() -> FakeSource {
    FakeSource {
        identities: vec![SOURCE],
        calls: Cell::new(0),
    }
}

/// A diff with every character the ledger has to escape.
fn fake_diff(base: &str) -> String {
    format!("diff against {base}\n-    1\n+    0 \"quoted\" \\ \tand\r\n")
}

impl Source for FakeSource {
    fn identity(&self) -> Result<String, String> {
        let call = self.calls.get();
        self.calls.set(call + 1);
        Ok(self.identities[call.min(self.identities.len() - 1)].into())
    }

    fn base(&self) -> Result<String, String> {
        Ok(BASE.into())
    }

    fn diff_against(&self, base: &str) -> Result<String, String> {
        Ok(fake_diff(base))
    }
}

fn example(id: &str, test: &str, channels: &[&str]) -> Example {
    Example {
        id: id.into(),
        statement: "walk_exit_status".into(),
        channels: channels.iter().map(|channel| channel.to_string()).collect(),
        file: "cli".into(),
        test: test.into(),
        line: 1,
        digest: format!("sha256:{id}"),
    }
}

fn expected(example: &str, channel: &str, outcome: Outcome) -> Run {
    Run {
        example: example.into(),
        digest: format!("sha256:{example}"),
        digest_version: DIGEST_VERSION,
        channel: channel.into(),
        source: SOURCE.into(),
        base: BASE.into(),
        outcome,
        diff: (outcome == Outcome::Fail).then(|| fake_diff(BASE)),
    }
}

fn temp_ledger(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "lemmaspec-intent-recorder-{label}-{}.intent",
        std::process::id()
    ));
    let _ = fs::remove_file(&path);
    path
}

fn read_ledger(path: &Path) -> Vec<Run> {
    runs::parse(&fs::read_to_string(path).unwrap(), RUNS_FILE).unwrap()
}

fn record_all(examples: &[Example], runner: &mut FakeRunner, ledger: &Path) -> Vec<Run> {
    record(
        examples,
        &Selection::default(),
        runner,
        &fake_source(),
        ledger,
    )
    .unwrap()
}

#[test]
fn one_passed_is_pass_one_failed_is_fail_and_one_ignored_is_skip() {
    let ledger = temp_ledger("outcomes");
    let examples = [
        example("passes", "passes", &["cli"]),
        example("fails", "fails", &["cli"]),
        example("ignored", "ignored", &["cli"]),
    ];
    let mut runner = FakeRunner::answering(&[
        ("passes", canned("pass")),
        ("fails", canned("fail")),
        ("ignored", canned("ignored")),
    ]);
    let recorded = record_all(&examples, &mut runner, &ledger);

    let runs = vec![
        expected("passes", "cli", Outcome::Pass),
        expected("fails", "cli", Outcome::Fail),
        expected("ignored", "cli", Outcome::Skip),
    ];
    assert_eq!(recorded, runs);
    assert_eq!(runs[1].diff.as_deref(), Some(fake_diff(BASE).as_str()));
    assert_eq!(read_ledger(&ledger), runs);
    assert!(
        !fs::read_to_string(&ledger).unwrap().contains('\r'),
        "line-ending rules must not be able to change a stored diff"
    );
    fs::remove_file(&ledger).unwrap();
}

// Covers AE18 (data half)
#[test]
fn a_build_failure_without_a_summary_line_is_recorded_as_error() {
    let ledger = temp_ledger("build-error");
    let mut runner = FakeRunner::answering(&[("breaks", canned("build_error"))]);
    let recorded = record_all(
        &[example("breaks", "breaks", &["cli"])],
        &mut runner,
        &ledger,
    );

    assert_eq!(recorded, [expected("breaks", "cli", Outcome::Error)]);
    assert_eq!(read_ledger(&ledger), recorded);
    fs::remove_file(&ledger).unwrap();
}

#[test]
fn a_run_matching_zero_tests_is_recorded_as_error() {
    let ledger = temp_ledger("no-match");
    let contradicted = RunOutput {
        success: false,
        ..canned("pass")
    };
    let mut runner = FakeRunner::answering(&[
        ("renamed", canned("no_match")),
        ("contradicted", contradicted),
    ]);
    let recorded = record_all(
        &[
            example("renamed", "renamed", &["cli"]),
            example("contradicted", "contradicted", &["cli"]),
        ],
        &mut runner,
        &ledger,
    );

    assert_eq!(
        recorded,
        [
            expected("renamed", "cli", Outcome::Error),
            expected("contradicted", "cli", Outcome::Error),
        ]
    );
    fs::remove_file(&ledger).unwrap();
}

#[test]
fn an_example_tagged_for_two_channels_runs_once_per_channel() {
    let ledger = temp_ledger("channels");
    let test = "exit_codes::open_exits_one";
    let mut runner = FakeRunner::answering(&[(test, canned("pass"))]);
    record_all(
        &[example("open_exits_one", test, &["cli", "library"])],
        &mut runner,
        &ledger,
    );

    let call = |channel: &str| ("cli".to_string(), test.to_string(), channel.to_string());
    assert_eq!(runner.calls, [call("cli"), call("library")]);
    assert_eq!(
        read_ledger(&ledger),
        [
            expected("open_exits_one", "cli", Outcome::Pass),
            expected("open_exits_one", "library", Outcome::Pass),
        ]
    );
    fs::remove_file(&ledger).unwrap();
}

#[test]
fn the_cargo_runner_runs_one_exact_test_with_the_channel_in_the_environment() {
    let runner = CargoTest {
        dir: PathBuf::from("project"),
    };
    let command = runner.command("cli", "exit_codes::open_exits_one", "library");

    assert_eq!(command.get_program(), "cargo");
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        [
            "test",
            "--locked",
            "--test",
            "cli",
            "--",
            "--exact",
            "exit_codes::open_exits_one"
        ]
    );
    assert_eq!(
        command.get_envs().collect::<Vec<_>>(),
        [(
            OsStr::new("LEMMASPEC_INTENT_CHANNEL"),
            Some(OsStr::new("library"))
        )]
    );
    assert_eq!(command.get_current_dir(), Some(Path::new("project")));
}

#[test]
fn recording_appends_entries_and_never_rewrites_existing_ones() {
    let existing = fs::read_to_string(format!("{FIXTURES}/runs/runs.intent")).unwrap();
    let before = runs::parse(&existing, RUNS_FILE).unwrap();
    assert_eq!(before.len(), 2);
    let ledger = temp_ledger("append");
    fs::write(&ledger, &existing).unwrap();
    let examples = [example("fails", "fails", &["cli"])];
    let mut runner = FakeRunner::answering(&[("fails", canned("fail"))]);

    let first = record_all(&examples, &mut runner, &ledger);
    let second = record_all(&examples, &mut runner, &ledger);

    let after = fs::read_to_string(&ledger).unwrap();
    assert!(after.starts_with(&existing), "{after}");
    assert_eq!(read_ledger(&ledger), [before, first, second].concat());
    fs::remove_file(&ledger).unwrap();
}

#[test]
fn appending_after_a_ledger_without_a_final_newline_keeps_it_parseable() {
    let ledger = temp_ledger("no-final-newline");
    fs::write(&ledger, "// no final newline").unwrap();
    let run = expected("fails", "cli", Outcome::Fail);

    runs::append(&ledger, std::slice::from_ref(&run)).unwrap();

    assert!(fs::read_to_string(&ledger)
        .unwrap()
        .starts_with("// no final newline"));
    assert_eq!(read_ledger(&ledger), [run]);
    fs::remove_file(&ledger).unwrap();
}

#[test]
fn a_selection_runs_only_the_examples_and_channels_it_names() {
    let examples = [
        example("a", "a", &["cli", "library"]),
        example("b", "b", &["cli"]),
    ];
    type Runs<'a> = &'a [(&'a str, &'a str)];
    let cases: [(Option<&str>, Option<&str>, Runs); 4] = [
        (None, None, &[("a", "cli"), ("a", "library"), ("b", "cli")]),
        (Some("a"), None, &[("a", "cli"), ("a", "library")]),
        (None, Some("cli"), &[("a", "cli"), ("b", "cli")]),
        (Some("a"), Some("library"), &[("a", "library")]),
    ];
    for (only_example, only_channel, expected_runs) in cases {
        let ledger = temp_ledger("selection");
        let mut runner = FakeRunner::answering(&[("a", canned("pass")), ("b", canned("pass"))]);
        let selection = Selection {
            example: only_example.map(Into::into),
            channel: only_channel.map(Into::into),
        };
        record(&examples, &selection, &mut runner, &fake_source(), &ledger).unwrap();

        let ran: Vec<(&str, &str)> = runner
            .calls
            .iter()
            .map(|(_, test, channel)| (test.as_str(), channel.as_str()))
            .collect();
        assert_eq!(ran, expected_runs, "{selection:?}");
        assert_eq!(read_ledger(&ledger).len(), expected_runs.len());
        fs::remove_file(&ledger).unwrap();
    }

    for (only_example, only_channel, message) in [
        (Some("z"), None, "unknown example z"),
        (
            Some("b"),
            Some("library"),
            "example b is not tagged for channel library",
        ),
        (None, Some("web"), "no example is tagged for channel web"),
    ] {
        let ledger = temp_ledger("refused-selection");
        let mut runner = FakeRunner::answering(&[]);
        let selection = Selection {
            example: only_example.map(Into::into),
            channel: only_channel.map(Into::into),
        };
        let error =
            record(&examples, &selection, &mut runner, &fake_source(), &ledger).unwrap_err();
        assert_eq!(error, message);
        assert!(runner.calls.is_empty());
        assert!(!ledger.exists());
    }
}

#[test]
fn recording_refuses_when_the_source_changes_while_examples_run() {
    let ledger = temp_ledger("source-changed");
    let mut runner = FakeRunner::answering(&[("passes", canned("pass"))]);
    let source = FakeSource {
        identities: vec![SOURCE, "sha256:edited"],
        calls: Cell::new(0),
    };

    let error = record(
        &[example("passes", "passes", &["cli"])],
        &Selection::default(),
        &mut runner,
        &source,
        &ledger,
    )
    .unwrap_err();

    assert!(error.contains("changed while recording"), "{error}");
    assert!(!ledger.exists());
}

#[test]
fn the_ledger_reader_rejects_malformed_entries_with_their_location() {
    let valid = "run {\n  example: a\n  digest: \"sha256:d\"\n  digest_version: 1\n  channel: cli\n  source: \"sha256:s\"\n  base: \"3bf8c7d\"\n  outcome: pass\n}\n";
    assert_eq!(
        runs::parse(valid, RUNS_FILE).unwrap(),
        [Run {
            example: "a".into(),
            digest: "sha256:d".into(),
            digest_version: 1,
            channel: "cli".into(),
            source: "sha256:s".into(),
            base: "3bf8c7d".into(),
            outcome: Outcome::Pass,
            diff: None,
        }]
    );

    let cases = [
        (
            valid.replace("run {", "approval {"),
            "runs.intent:1: unknown block approval; the run ledger holds only run blocks",
        ),
        (
            valid.replace("run {", "run named {"),
            "runs.intent:1: a run block takes no name",
        ),
        (
            valid.replace("  outcome: pass\n", ""),
            "runs.intent:1: run needs outcome",
        ),
        (
            valid.replace("outcome: pass", "outcome: pass\n  flaky: 1"),
            "runs.intent:9: run has no field flaky",
        ),
        (
            valid.replace("outcome: pass", "outcome: pass\n  outcome: fail"),
            "runs.intent:9: duplicate field outcome in run",
        ),
        (
            valid.replace("channel: cli", "channel: \"cli\""),
            "runs.intent:5: channel must be an identifier",
        ),
        (
            valid.replace("digest: \"sha256:d\"", "digest: sha256"),
            "runs.intent:3: digest must be a string",
        ),
        (
            valid.replace("digest_version: 1", "digest_version: \"1\""),
            "runs.intent:4: digest_version must be an integer",
        ),
        (
            valid.replace("outcome: pass", "outcome: flaky"),
            "runs.intent:8: unknown outcome flaky; expected pass, fail, error or skip",
        ),
        (
            valid.replace("outcome: pass", "outcome: pass\n  diff: \"x\""),
            "runs.intent:9: only a failing run stores a diff",
        ),
        (
            valid.replace("outcome: pass", "outcome: fail"),
            "runs.intent:1: a failing run needs diff",
        ),
        (valid.replace('}', ""), "runs.intent:8: expected field name"),
    ];
    for (source, message) in cases {
        let error = runs::parse(&source, RUNS_FILE).unwrap_err();
        assert!(error.starts_with(message), "{error}\n{source}");
    }
}

#[test]
fn reading_a_catalog_ledger_treats_a_missing_one_as_empty() {
    assert_eq!(runs::read(&BTreeMap::new()).unwrap(), []);
    let catalog = DirReader(format!("{FIXTURES}/catalog/intent").into());
    assert_eq!(runs::read(&catalog).unwrap(), []);
    let fixture = DirReader(format!("{FIXTURES}/runs").into());
    assert_eq!(runs::read(&fixture).unwrap().len(), 2);
    let broken = BTreeMap::from([(RUNS_FILE.to_string(), "run {".to_string())]);
    assert!(runs::read(&broken)
        .unwrap_err()
        .starts_with("runs.intent:1:"));
}

/// A fresh git repository under the system temp directory, removed on drop.
struct TempRepo {
    dir: PathBuf,
}

impl TempRepo {
    fn with_project(label: &str) -> TempRepo {
        let dir = std::env::temp_dir().join(format!(
            "lemmaspec-intent-recorder-{label}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let repo = TempRepo { dir };
        repo.git(&["init", "-q"]);
        for (key, value) in [
            ("user.email", "intent@example.com"),
            ("user.name", "Intent Test"),
            ("commit.gpgsign", "false"),
            ("core.autocrlf", "false"),
            ("core.hooksPath", "no-hooks"),
        ] {
            repo.git(&["config", key, value]);
        }
        repo.write("src/lib.rs", "pub fn follow_up() -> u8 {\n    1\n}\n");
        repo.write("intent/catalog.intent", "catalog fixture {\n}\n");
        repo.write("intent/runs.intent", "// runs\n");
        repo.write("intent/approvals.intent", "// approvals\n");
        repo
    }

    fn git(&self, args: &[&str]) -> String {
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

    fn write(&self, path: &str, content: &str) {
        let path = self.dir.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn commit(&self) -> String {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "--no-verify", "-m", "base"]);
        self.git(&["rev-parse", "HEAD"]).trim().to_string()
    }

    fn source(&self) -> Repo {
        Repo::of_catalog(&self.dir.join("intent")).unwrap()
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

// Covers AE10 (data half)
// Covers AE20 (data half)
#[test]
fn a_run_on_a_dirty_scratch_tree_stores_that_trees_identity_its_base_and_its_diff() {
    let repo = TempRepo::with_project("scratch");
    let base = repo.commit();
    let source = repo.source();
    let clean = source.commit_identity(&base).unwrap();
    let ledger = repo.dir.join("intent").join(RUNS_FILE);
    let examples = [example("follow_up", "follow_up", &["mcp"])];

    repo.write("src/lib.rs", "pub fn follow_up() -> u8 {\n    0\n}\n");
    let scratch = source.working_tree_identity().unwrap();
    assert_ne!(scratch, clean);
    let mut runner = FakeRunner::answering(&[("follow_up", canned("fail"))]);
    let red = record(
        &examples,
        &Selection::default(),
        &mut runner,
        &source,
        &ledger,
    )
    .unwrap();
    assert_eq!(red.len(), 1);
    assert_eq!(red[0].outcome, Outcome::Fail);
    assert_eq!(red[0].source, scratch);
    assert_eq!(red[0].base, base);
    let diff = red[0].diff.as_deref().unwrap();
    assert!(diff.contains("-    1\n+    0\n"), "{diff}");
    assert!(!diff.contains(RUNS_FILE), "{diff}");

    repo.write("src/lib.rs", "pub fn follow_up() -> u8 {\n    1\n}\n");
    let mut runner = FakeRunner::answering(&[("follow_up", canned("pass"))]);
    let green = record(
        &examples,
        &Selection::default(),
        &mut runner,
        &source,
        &ledger,
    )
    .unwrap();
    assert_eq!(green.len(), 1);
    assert_eq!(green[0].outcome, Outcome::Pass);
    assert_eq!(green[0].source, clean);
    assert_eq!(green[0].base, base);
    assert_eq!(green[0].diff, None);

    assert!(
        source.matches_head().unwrap(),
        "the ledger the runs went to is not source"
    );
    assert_eq!(read_ledger(&ledger), [red, green].concat());
}

#[test]
fn an_explicit_ledger_receives_the_runs_and_the_catalog_ledger_stays_untouched() {
    let repo = TempRepo::with_project("explicit-ledger");
    repo.commit();
    let catalog_ledger = repo.dir.join("intent").join(RUNS_FILE);
    let before = fs::read(&catalog_ledger).unwrap();
    let ledger = temp_ledger("explicit-ledger");
    let mut runner = FakeRunner::answering(&[("follow_up", canned("pass"))]);

    let recorded = record(
        &[example("follow_up", "follow_up", &["cli"])],
        &Selection::default(),
        &mut runner,
        &repo.source(),
        &ledger,
    )
    .unwrap();

    assert_eq!(fs::read(&catalog_ledger).unwrap(), before);
    assert_eq!(read_ledger(&ledger), recorded);
    fs::remove_file(&ledger).unwrap();
}
