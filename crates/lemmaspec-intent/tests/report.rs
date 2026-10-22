mod common;

use std::collections::BTreeMap;

use common::{cited, root_catalog, Project, STATEMENTS};
use lemmaspec::{parse_artifact, print_artifact, EvidenceKind, FactValue};
use lemmaspec_intent::{
    digest::DIGEST_VERSION,
    loader::RUNS_FILE,
    project::{self as projection, Inputs},
    runs::{self, Outcome, Run},
};

fn verified(label: &str) -> Project {
    let project = Project::new(label);
    project.approve_all();
    project.record_red();
    project
}

fn standings(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(id, standing)| (id.to_string(), standing.to_string()))
        .collect()
}

fn section<'a>(markdown: &'a str, heading: &str) -> &'a str {
    let start = markdown
        .find(&format!("\n## {heading}"))
        .unwrap_or_else(|| panic!("no {heading} section in\n{markdown}"));
    let rest = &markdown[start + 1..];
    let end = rest[3..].find("\n## ").map_or(rest.len(), |end| end + 3);
    &rest[..end]
}

#[test]
fn projecting_the_same_inputs_twice_gives_byte_identical_evidence_with_bare_ids_and_snapshot_bases()
{
    let project = verified("determinism");
    let green = project.record_green("green.intent");
    let identity = project.identity();
    let evidence = || {
        let inputs = Inputs::read(
            &project.intent(),
            &project.tests(),
            std::slice::from_ref(&green),
        )
        .unwrap();
        projection::project(&inputs, &identity)
    };
    let text = print_artifact(&evidence());
    assert_eq!(text, print_artifact(&evidence()));
    let evidence = evidence();
    assert_eq!(parse_artifact(&text).unwrap(), evidence);

    let mut counts = BTreeMap::new();
    for fact in &evidence.facts {
        let index = counts.entry(fact.relation.as_str()).or_insert(0);
        assert_eq!(fact.id, format!("{}_{index}", fact.relation));
        *index += 1;
        let basis = fact.basis.as_ref().unwrap();
        assert_eq!(basis.kind, EvidenceKind::Snapshot, "{fact:?}");
        assert!(basis.identity.as_ref().unwrap().starts_with("sha256:"));
    }
    assert!(evidence.facts.iter().any(|fact| fact
        .args
        .iter()
        .any(|arg| matches!(arg, FactValue::Symbol(digest) if digest.starts_with("sha256:")))));
    assert!(evidence
        .facts
        .iter()
        .any(|fact| fact.relation == "scoped_to"
            && fact.args[1] == FactValue::Symbol("library".into())));
    let expectations: Vec<_> = evidence
        .expectations
        .iter()
        .map(|e| (e.query.as_str(), e.count))
        .collect();
    assert_eq!(
        expectations,
        [
            ("awaiting_approval(I)", 0),
            ("standing(\"library_returns_report\", Kind)", 1),
            ("standing(\"results_are_honest\", Kind)", 1),
            ("standing(\"walk_exit_status\", Kind)", 1),
        ]
    );

    let report = project.report(std::slice::from_ref(&green), None);
    assert_eq!(report.markdown, project.report(&[green], None).markdown);
    let html = report.html(None).unwrap();
    assert!(html.contains(&report.target));
    assert!(!html.contains("Not current"), "{html}");
}

#[test]
fn an_unapproved_definition_makes_the_report_pending_and_is_listed_first() {
    let project = Project::new("unapproved");
    project.approve(
        &[
            "cli",
            "library",
            "results_are_honest",
            "library_returns_report",
            "open_exits_one",
            "library_report",
        ],
        "owner",
    );
    let report = project.report(&[], None);
    assert!(report.pending);
    assert_eq!(report.standings["walk_exit_status"], "unapproved");
    let awaiting = section(&report.markdown, "Awaiting approval");
    assert_eq!(
        awaiting,
        "## Awaiting approval\n\n- `walk_exit_status` (definition): never approved, or re-added after an approved removal\n"
    );
    assert!(report.markdown.find("## Awaiting approval") < report.markdown.find("## Statements"));
}

#[test]
fn a_fully_approved_catalog_with_unverified_definitions_is_not_pending_and_shows_reasons() {
    let project = Project::new("unverified");
    project.approve_all();
    let report = project.report(&[], None);
    assert!(!report.pending, "{}", report.markdown);
    assert_eq!(
        report.standings,
        standings(&[
            ("library_returns_report", "unverified"),
            ("results_are_honest", "unverified"),
            ("walk_exit_status", "unverified"),
        ])
    );
    assert!(report
        .markdown
        .contains("## Awaiting approval\n\nNothing awaits approval.\n"));
    let statements = section(&report.markdown, "Statements");
    for reason in [
        "- Reason: an example has no failing run with its current text on this channel (`cli`)",
        "- Reason: an example does not pass cleanly at the target on this channel (`cli`)",
        "- Reason: a definition or exception it requires is not verified (`walk_exit_status`)",
    ] {
        assert!(statements.contains(reason), "{reason}\n{statements}");
    }
}

#[test]
fn html_and_markdown_show_each_statements_prose_links_and_standing() {
    let project = verified("rendered");
    let green = project.record_green("green.intent");
    let report = project.report(&[green], None);
    let statements = section(&report.markdown, "Statements");
    let catalog = project.intent().join("catalog.intent");
    assert!(statements.contains(&format!(
        "### `walk_exit_status` (definition): verified\n\n> walk exits 0 when every expectation holds, 1 when one is open, and 2 when the artifact is invalid.\n\n- File: `{}`\n- Refines: `results_are_honest`\n- Examples: `open_exits_one` on `cli`\n",
        cited(&catalog)
    )), "{statements}");
    assert!(statements.contains(
        "- Narrows: `walk_exit_status`\n- Kind: holds_instead on `library`\n- Examples: `library_report` on `library`\n"
    ), "{statements}");
    let html = report.html(None).unwrap();
    for prose in [
        "LemmaSpec never reports an artifact as passing while one of its claims is open.",
        "The library returns the report value, and its status field carries the same outcomes.",
        "stands verified",
        "refines",
    ] {
        assert!(html.contains(prose), "{prose}");
    }
}

#[test]
fn a_green_run_only_in_an_extra_ledger_verifies_a_definition() {
    let project = verified("extra-ledger");
    let committed = project.report(&[], None);
    assert_eq!(committed.standings["walk_exit_status"], "unverified");
    let green = project.record_green("green.intent");
    let missing = project.dir.join("ci").join("missing.intent");
    let report = project.report(&[green, missing], None);
    assert_eq!(
        report.standings,
        standings(&[
            ("library_returns_report", "verified"),
            ("results_are_honest", "verified"),
            ("walk_exit_status", "verified"),
        ])
    );
    assert!(!report.pending);
}

#[test]
fn the_header_names_the_evaluated_identity_and_whether_it_matches_head() {
    let project = Project::new("header");
    let head = project.commit("head");
    let clean = project.report(&[], None);
    assert!(clean.markdown.starts_with(&format!(
        "# Intent report\n\nSource identity `{}` matches the tree of HEAD commit `{head}`.\n",
        clean.target
    )));
    project.edit("src/lib.rs", "    1", "    0");
    let dirty = project.report(&[], None);
    assert_ne!(dirty.target, clean.target);
    assert_eq!(dirty.target, project.identity());
    assert!(dirty.markdown.starts_with(&format!(
        "# Intent report\n\nSource identity `{}` is the working tree, which does not match the tree of HEAD commit `{head}`, so these standings do not hold for that commit.\n",
        dirty.target
    )), "{}", dirty.markdown);
}

// Covers AE11
#[test]
fn approvals_added_since_the_base_are_listed_with_their_approver_and_an_edited_log_is_altered() {
    let project = Project::new("ae11");
    project.approve_all();
    let base = project.commit("approved");
    project.edit(
        "intent/catalog.intent",
        "when one is open",
        "when an expectation is open",
    );
    project.approve(&["walk_exit_status"], "agent");
    project.commit("changes the definition and approves it");
    let report = project.report(&[], Some(&base));
    let since = section(&report.markdown, &format!("Since base `{base}`"));
    assert!(
        since
            .contains("### Approvals\n\n- Entry 8: agent approved definition `walk_exit_status`\n"),
        "{since}"
    );
    assert!(!since.contains("Entry 7"), "{since}");

    project.edit(
        "intent/approvals.intent",
        "by: \"owner\"",
        "by: \"someone else\"",
    );
    let report = project.report(&[], Some(&base));
    let since = section(&report.markdown, &format!("Since base `{base}`"));
    assert!(since.contains("The approval log was altered"), "{since}");
    assert!(!since.contains("Entry 8"), "{since}");
}

#[test]
fn a_base_without_an_approval_log_lists_every_entry_as_added() {
    let project = Project::new("no-base-log");
    let base = project.commit("no approvals yet");
    project.approve_all();
    let report = project.report(&[], Some(&base));
    let since = section(&report.markdown, &format!("Since base `{base}`"));
    assert_eq!(since.matches("\n- Entry ").count(), 7, "{since}");
    assert!(since.contains("- Entry 1: owner approved channel `cli`\n"));
    assert!(since.contains("- Entry 7: owner approved example `library_report`\n"));
    assert!(since.contains("### Failing runs\n\nNone.\n"), "{since}");
}

// Covers AE20
#[test]
fn a_failing_run_recorded_since_the_base_is_listed_with_the_diff_of_its_break() {
    let project = Project::new("ae20");
    project.approve_all();
    let base = project.commit("approved");
    project.edit("src/lib.rs", "    1", "    0");
    let scratch = project.identity();
    let diff = project.repo().diff(&base).unwrap();
    assert!(diff.contains("+    0"), "{diff}");
    project.record(
        &project.intent().join(RUNS_FILE),
        Outcome::Fail,
        &scratch,
        &diff,
    );
    project.edit("src/lib.rs", "    0", "    1");
    project.commit("records red on a scratch break");
    let report = project.report(&[], Some(&base));
    let since = section(&report.markdown, &format!("Since base `{base}`"));
    assert!(
        since.contains(&format!(
            "`open_exits_one` failed on `cli` at source `{scratch}`, based on `0000000`:\n\n```diff\n{diff}```\n"
        )),
        "{since}"
    );
}

// Covers AE16
#[test]
fn moving_statements_into_an_imported_file_leaves_every_standing_unchanged_and_nothing_pending() {
    let project = verified("ae16");
    let before = project.report(&[project.record_green("before.intent")], None);
    assert!(!before.pending, "{}", before.markdown);
    assert_eq!(
        before.standings,
        standings(&[
            ("library_returns_report", "verified"),
            ("results_are_honest", "verified"),
            ("walk_exit_status", "verified"),
        ])
    );
    project.write(
        "intent/catalog.intent",
        "catalog fixture {\n  use \"./channels.intent\" as channels\n  use \"./walk.intent\" as walk\n}\n",
    );
    project.write(
        "intent/walk.intent",
        &root_catalog(STATEMENTS).replace("fixture", "walk"),
    );
    // The source changed, so CI records green again at the new identity.
    let after = project.report(&[project.record_green("after.intent")], None);
    assert_ne!(after.target, before.target);
    assert_eq!(after.standings, before.standings);
    assert!(!after.pending, "{}", after.markdown);
    assert!(after.markdown.contains(&format!(
        "- File: `{}`",
        cited(&project.intent().join("walk.intent"))
    )));
}

// Covers AE19
#[test]
fn a_test_tagged_with_an_unknown_statement_is_listed_as_ignored_and_the_report_still_renders() {
    let project = verified("ae19");
    project.write(
        "tests/ghost.rs",
        "/// intent: example ghost exemplifies no_such_statement on cli\n#[test]\nfn ghost() {}\n",
    );
    let report = project.report(&[project.record_green("green.intent")], None);
    let ignored = section(&report.markdown, "Ignored examples");
    assert_eq!(
        ignored,
        format!(
            "## Ignored examples\n\n- `ghost` at `{}:1` (`ghost`): unknown statement no_such_statement\n",
            cited(&project.tests().join("ghost.rs"))
        )
    );
    assert_eq!(report.standings["walk_exit_status"], "verified");
    assert!(!report.pending);
    assert!(!report
        .bound
        .facts
        .iter()
        .any(|fact| fact.args.contains(&FactValue::Symbol("ghost".into()))));
    assert!(report.html(None).is_ok());
}

#[test]
fn approvals_and_runs_recorded_under_another_digest_version_are_listed_and_not_evaluated() {
    let project = verified("digest-version");
    let log = project.read("intent/approvals.intent");
    project.write(
        "intent/approvals.intent",
        &format!("{log}approval {{\n  item: walk_exit_status\n  kind: definition\n  action: remove\n  by: \"old tool\"\n  digest: \"sha256:old\"\n  digest_version: 0\n}}\n"),
    );
    let current = project.report(&[], None);
    let digest = current
        .bound
        .facts
        .iter()
        .find(|fact| {
            fact.relation == "current_digest"
                && fact.args[0] == FactValue::Symbol("open_exits_one".into())
        })
        .map(|fact| fact.args[1].clone())
        .unwrap();
    let FactValue::Symbol(digest) = digest else {
        unreachable!()
    };
    runs::append(
        &project.intent().join(RUNS_FILE),
        &[Run {
            example: "open_exits_one".into(),
            digest,
            digest_version: DIGEST_VERSION - 1,
            channel: "cli".into(),
            source: project.identity(),
            base: "0000000".into(),
            outcome: Outcome::Pass,
            diff: None,
        }],
    )
    .unwrap();
    let report = project.report(&[], None);
    assert_eq!(report.standings["walk_exit_status"], "unverified");
    assert!(!report.pending, "{}", report.markdown);
    let skipped = section(&report.markdown, "Not evaluated");
    assert!(skipped.contains(
        "- Approval entry 8 for `walk_exit_status` by old tool: recorded under digest version 0\n"
    ), "{skipped}");
    assert!(
        skipped.contains(&format!(
            "- Run of `open_exits_one` on `cli` in `{}`: recorded under digest version 0\n",
            cited(&project.intent().join(RUNS_FILE))
        )),
        "{skipped}"
    );
    assert!(!report
        .bound
        .facts
        .iter()
        .any(|fact| fact.relation == "run" && fact.args[4] == FactValue::Symbol("pass".into())));
}

#[test]
fn an_approved_statement_deleted_from_the_catalog_keeps_one_unapproved_standing() {
    let project = verified("removed");
    project.write("intent/catalog.intent", &root_catalog(&STATEMENTS.replace(
        "  exception library_returns_report {\n    narrows: walk_exit_status\n    channels: [ch.library]\n    kind: holds_instead\n    text: \"The library returns the report value, and its status field carries the same outcomes.\"\n  }\n",
        "",
    )));
    let report = project.report(&[], None);
    assert!(report.pending);
    assert_eq!(report.standings["library_returns_report"], "unapproved");
    assert!(report
        .bound
        .expectations
        .iter()
        .any(|e| e.query == "standing(\"library_returns_report\", Kind)" && e.count == 1));
    let statements = section(&report.markdown, "Statements");
    assert!(statements.contains(
        "### `library_returns_report` (exception): unapproved\n\n> The library returns the report value, and its status field carries the same outcomes.\n\n- File: removed from the catalog; its approval stands until a removal is approved\n- Narrows: `walk_exit_status`\n- Reason: removed without an approved removal\n"
    ), "{statements}");
}
