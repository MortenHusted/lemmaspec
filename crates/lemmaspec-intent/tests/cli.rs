mod common;

use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

use common::{Project, ITEMS};
use lemmaspec::walk_artifact;
use lemmaspec_intent::CHECKER;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/catalog");

fn intent_in(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lemmaspec-intent"))
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
}

fn intent(args: &[&str]) -> Output {
    intent_in(Path::new(FIXTURES), args)
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

#[test]
fn check_exits_zero_on_a_valid_catalog_and_names_the_error_otherwise() {
    for valid in [intent(&["check"]), intent(&["check", "--dir", "intent"])] {
        assert!(valid.status.success(), "{}", stderr(&valid));
    }
    for (args, error) in [
        (
            &["check", "--dir", "unknown_reference"][..],
            "catalog.intent:6: unknown reference missing_definition in narrows of orphan",
        ),
        (
            &["check", "--dir", "outside"],
            "catalog.intent:2: import \"../escape_target.intent\" must be a relative path inside the catalog directory",
        ),
        (
            &["check", "--dir", "unreachable"],
            "sub/stray.intent: not imported from catalog.intent",
        ),
        (
            &["check", "--dir", "missing"],
            "catalog.intent not found in the catalog directory",
        ),
        (
            &["check", "--dir", "intent", "--dir", "intent"],
            "unknown or duplicate option --dir",
        ),
        (&["verify"], "unknown command verify"),
    ] {
        let run = intent(args);
        assert_eq!(run.status.code(), Some(2), "{args:?}");
        assert_eq!(stderr(&run), format!("lemmaspec-intent: {error}\n"));
    }
}

#[test]
fn report_exits_one_while_changes_await_approval_and_zero_once_approved_even_if_unverified() {
    let project = Project::new("cli-exit");
    let pending = intent_in(&project.dir, &["report"]);
    assert_eq!(pending.status.code(), Some(1), "{}", stderr(&pending));
    let report = stdout(&pending);
    assert!(report.starts_with("# Intent report\n"), "{report}");
    assert!(report.contains("- `walk_exit_status` (definition): never approved"));
    assert!(
        report.contains("- File: `intent/catalog.intent`"),
        "{report}"
    );

    let mut approve = vec!["approve"];
    approve.extend(ITEMS);
    approve.extend(["--by", "owner"]);
    let approved = intent_in(&project.dir, &approve);
    assert!(approved.status.success(), "{}", stderr(&approved));
    assert!(project
        .read("intent/approvals.intent")
        .contains("by: \"owner\""));

    let unverified = intent_in(&project.dir, &["report"]);
    assert_eq!(unverified.status.code(), Some(0), "{}", stderr(&unverified));
    assert!(stdout(&unverified).contains("### `walk_exit_status` (definition): unverified"));
}

#[test]
fn reconfirm_and_remove_append_to_the_log_and_refuse_without_an_approver_or_an_id() {
    let project = Project::new("cli-approvals");
    for args in [
        &["approve", "walk_exit_status"][..],
        &["approve", "--by", "owner"],
        &["approve", "walk_exit_status", "--by"],
        &["reconfirm", "walk_exit_status", "--by", "owner"],
        &["remove", "walk_exit_status", "--by", "owner"],
    ] {
        let refused = intent_in(&project.dir, args);
        assert_eq!(refused.status.code(), Some(2), "{args:?}");
        assert!(
            !project.intent().join("approvals.intent").exists(),
            "{args:?}"
        );
    }
    project.approve_all();
    let before = project.read("intent/approvals.intent");
    let reconfirmed = intent_in(
        &project.dir,
        &["reconfirm", "open_exits_one", "--by", "agent"],
    );
    assert!(reconfirmed.status.success(), "{}", stderr(&reconfirmed));
    let removed = intent_in(
        &project.dir,
        &[
            "remove",
            "open_exits_one",
            "library_report",
            "--by",
            "agent",
        ],
    );
    assert!(removed.status.success(), "{}", stderr(&removed));
    let log = project.read("intent/approvals.intent");
    assert!(log.starts_with(&before));
    assert_eq!(log[before.len()..].matches("by: \"agent\"").count(), 3);
    assert_eq!(log[before.len()..].matches("action: remove").count(), 2);
}

#[test]
fn report_writes_markdown_html_and_the_bound_artifact_to_the_paths_given() {
    let project = Project::new("cli-outputs");
    project.approve_all();
    project.record_red();
    let green = project.record_green("green.intent");
    let green = green.to_str().unwrap();
    fs::create_dir_all(project.dir.join("out")).unwrap();
    let html = intent_in(
        &project.dir,
        &[
            "report",
            "--ledger",
            green,
            "--format",
            "html",
            "-o",
            "out/report.html",
            "--bound",
            "out/bound.lemmaspec",
        ],
    );
    assert_eq!(html.status.code(), Some(0), "{}", stderr(&html));
    assert!(stdout(&html).is_empty());
    let page = project.read("out/report.html");
    assert!(page.contains("<html"), "{page}");
    assert!(page.contains("href=\"../intent/catalog.intent\""), "{page}");
    let bound = walk_artifact(&project.read("out/bound.lemmaspec")).unwrap();
    assert_eq!(bound.status, "clean");
    assert!(bound
        .facts
        .iter()
        .any(|fact| fact.relation == "standing" && fact.args == ["walk_exit_status", "verified"]));

    let markdown = intent_in(
        &project.dir,
        &["report", "--ledger", green, "-o", "out/report.md"],
    );
    assert_eq!(markdown.status.code(), Some(0), "{}", stderr(&markdown));
    assert!(project
        .read("out/report.md")
        .contains("### `walk_exit_status` (definition): verified"));
}

#[test]
fn invalid_options_leave_existing_output_files_untouched() {
    let project = Project::new("cli-invalid");
    project.write("out.md", "previous report\n");
    project.write("bound.lemmaspec", "previous bound\n");
    for args in [
        &[
            "report",
            "-o",
            "out.md",
            "--bound",
            "bound.lemmaspec",
            "--format",
            "pdf",
        ][..],
        &[
            "report",
            "-o",
            "out.md",
            "--bound",
            "bound.lemmaspec",
            "--frobnicate",
        ],
        &[
            "report",
            "-o",
            "out.md",
            "--bound",
            "bound.lemmaspec",
            "--base",
            "no-such-commit",
        ],
        &[
            "report",
            "-o",
            "out.md",
            "--bound",
            "bound.lemmaspec",
            "--dir",
            "missing",
        ],
        &[
            "report",
            "-o",
            "out.md",
            "--bound",
            "bound.lemmaspec",
            "-o",
            "other.md",
        ],
        &[
            "report",
            "-o",
            "out.md",
            "--bound",
            "bound.lemmaspec",
            "--ledger",
        ],
    ] {
        let run = intent_in(&project.dir, args);
        assert_eq!(run.status.code(), Some(2), "{args:?}: {}", stderr(&run));
        assert!(stderr(&run).starts_with("lemmaspec-intent: "), "{args:?}");
        assert_eq!(project.read("out.md"), "previous report\n", "{args:?}");
        assert_eq!(
            project.read("bound.lemmaspec"),
            "previous bound\n",
            "{args:?}"
        );
    }
}

#[test]
fn record_with_no_examples_creates_no_ledger_and_the_report_reads_a_missing_one_as_empty() {
    let project = Project::new("cli-record");
    fs::remove_file(project.tests().join("walk.rs")).unwrap();
    let recorded = intent_in(&project.dir, &["record", "--ledger", "ci/runs.intent"]);
    assert!(recorded.status.success(), "{}", stderr(&recorded));
    assert!(!project.dir.join("ci/runs.intent").exists());
    let report = intent_in(&project.dir, &["report", "--ledger", "ci/runs.intent"]);
    assert_eq!(report.status.code(), Some(1), "{}", stderr(&report));

    let unknown = intent_in(&project.dir, &["record", "--example", "ghost"]);
    assert_eq!(unknown.status.code(), Some(2));
    assert_eq!(
        stderr(&unknown),
        "lemmaspec-intent: unknown example ghost\n"
    );
}

#[test]
fn checker_prints_the_bundled_checker_and_help_prints_the_usage() {
    let checker = intent(&["checker"]);
    assert!(checker.status.success());
    assert_eq!(stdout(&checker), CHECKER);
    let help = intent(&["--help"]);
    assert!(help.status.success());
    assert!(stdout(&help).starts_with("usage: lemmaspec-intent"));
    let command_help = intent(&["record", "--help"]);
    assert!(command_help.status.success(), "{}", stderr(&command_help));
    assert_eq!(stdout(&command_help), stdout(&help));
    for command in [
        "check",
        "approve",
        "reconfirm",
        "remove",
        "record",
        "report",
        "checker",
        "intro",
    ] {
        assert!(
            stdout(&help).contains(&format!("lemmaspec-intent {command}")),
            "{command}"
        );
    }
}

#[test]
fn intro_prints_the_agent_workflow_and_names_the_tool_written_files() {
    let intro = intent(&["intro"]);
    assert!(intro.status.success(), "{}", stderr(&intro));
    let text = stdout(&intro);
    for expected in [
        "intent/approvals.intent",
        "intent/runs.intent",
        "LEMMASPEC_INTENT_CHANNEL",
    ] {
        assert!(text.contains(expected), "intro lacks {expected:?}:\n{text}");
    }
}
