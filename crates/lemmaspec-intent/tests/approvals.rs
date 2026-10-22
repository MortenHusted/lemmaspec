use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use lemmaspec_intent::{
    approvals::{self, Action, Entry, ItemKind, Log},
    digest::{statement_digest, DIGEST_VERSION},
    examples::{scan, Scan},
    loader::{load, DirReader, APPROVALS_FILE},
    Catalog,
};

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/approvals");

const CATALOG: &str = "intent/catalog.intent";
const TESTS: &str = "tests/walk.rs";

const DEFINITION_TEXT: &str =
    "walk exits 0 when every expectation holds, 1 when one is open, and 2 when the artifact is invalid.";
const CHANGED_TEXT: &str = "walk exits 0 when every expectation holds and 1 otherwise.";

const DEFINITION: &str = r#"  definition walk_exit_status {
    refines: results_are_honest
    text: "walk exits 0 when every expectation holds, 1 when one is open, and 2 when the artifact is invalid."
  }
"#;

const EXCEPTION: &str = r#"  exception library_returns_report {
    narrows: walk_exit_status
    channels: [library]
    kind: holds_instead
    text: "The library returns the report value, and its status field carries the same outcomes."
  }
"#;

const OPEN_EXITS_ONE: &str = r#"/// intent: example open_exits_one exemplifies walk_exit_status on library, cli
#[test]
fn open_expectation_exits_one() {
    assert_eq!(1, 1);
}
"#;

/// A scratch copy of the fixture project: the catalog and its approval log in
/// `intent/`, the tagged examples in `tests/`.
struct Project(PathBuf);

impl Project {
    fn new(label: &str) -> Project {
        let root = std::env::temp_dir().join(format!(
            "lemmaspec-intent-approvals-{label}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        for dir in ["intent", "tests"] {
            fs::create_dir_all(root.join(dir)).unwrap();
            for entry in fs::read_dir(Path::new(FIXTURE).join(dir)).unwrap() {
                let path = entry.unwrap().path();
                fs::copy(&path, root.join(dir).join(path.file_name().unwrap())).unwrap();
            }
        }
        Project(root)
    }

    fn intent(&self) -> PathBuf {
        self.0.join("intent")
    }

    fn tests(&self) -> PathBuf {
        self.0.join("tests")
    }

    fn write(&self, path: &str, source: &str) {
        fs::write(self.0.join(path), source).unwrap();
    }

    /// Replace `from` with `to` in the project file at `path`.
    fn edit(&self, path: &str, from: &str, to: &str) {
        let source = fs::read_to_string(self.0.join(path)).unwrap();
        assert!(source.contains(from), "{from:?} is not in {path}");
        self.write(path, &source.replace(from, to));
    }

    fn catalog(&self) -> Catalog {
        load(&DirReader(self.intent())).unwrap()
    }

    fn examples(&self) -> Scan {
        scan(&self.tests(), &self.catalog()).unwrap()
    }

    fn statement_digest(&self, id: &str) -> String {
        statement_digest(&self.catalog().statements[id])
    }

    fn example_digest(&self, id: &str) -> String {
        let examples = self.examples().examples;
        examples.into_iter().find(|e| e.id == id).unwrap().digest
    }

    /// The log's bytes; empty while it does not exist.
    fn log_source(&self) -> String {
        fs::read_to_string(self.intent().join(APPROVALS_FILE)).unwrap_or_default()
    }

    fn log(&self) -> Log {
        Log::read(&DirReader(self.intent())).unwrap()
    }

    fn append(
        &self,
        change: impl FnOnce(&Log, &Catalog, &Scan) -> Result<String, String>,
    ) -> Result<(), String> {
        approvals::append(&self.intent(), &self.tests(), change)
    }

    fn approve(&self, ids: &[&str]) -> Result<(), String> {
        self.append(|_, catalog, examples| approvals::approve(catalog, examples, ids, "owner"))
    }

    fn reconfirm(&self, ids: &[&str]) -> Result<(), String> {
        self.append(|log, catalog, examples| {
            approvals::reconfirm(log, catalog, examples, ids, "owner")
        })
    }

    fn remove(&self, ids: &[&str]) -> Result<(), String> {
        self.append(|log, catalog, examples| {
            approvals::remove(log, catalog, examples, ids, "owner")
        })
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn links(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(target, digest)| (target.to_string(), digest.to_string()))
        .collect()
}

#[test]
fn approving_a_definition_appends_its_digest_refined_intent_prose_and_approver_last() {
    let project = Project::new("definition");
    project.approve(&["results_are_honest"]).unwrap();
    project.approve(&["walk_exit_status"]).unwrap();
    let log = project.log();
    assert_eq!(log.entries.len(), 2, "{log:?}");
    assert_eq!(log.entries[0].item, "results_are_honest");
    assert_eq!(
        log.entries[1],
        Entry {
            item: "walk_exit_status".into(),
            kind: ItemKind::Definition,
            action: Action::Approve,
            by: "owner".into(),
            digest: project.statement_digest("walk_exit_status"),
            digest_version: DIGEST_VERSION,
            links: links(&[(
                "results_are_honest",
                &project.statement_digest("results_are_honest")
            )]),
            channels: vec![],
            text: Some(DEFINITION_TEXT.into()),
        }
    );
}

#[test]
fn approving_an_example_stores_its_statement_channels_and_digest() {
    let project = Project::new("example");
    project.approve(&["open_exits_one"]).unwrap();
    assert_eq!(
        project.log().entries,
        [Entry {
            item: "open_exits_one".into(),
            kind: ItemKind::Example,
            action: Action::Approve,
            by: "owner".into(),
            digest: project.example_digest("open_exits_one"),
            digest_version: DIGEST_VERSION,
            links: links(&[(
                "walk_exit_status",
                &project.statement_digest("walk_exit_status")
            )]),
            channels: vec!["cli".into(), "library".into()],
            text: None,
        }]
    );
}

#[test]
fn an_entry_is_one_approval_block_with_one_field_per_line_and_escaped_prose() {
    let project = Project::new("format");
    project.edit(
        CATALOG,
        "LemmaSpec never reports",
        r#"LemmaSpec \"never\" reports\n\t\\"#,
    );
    project
        .approve(&["results_are_honest", "open_exits_one"])
        .unwrap();
    let intent = project.statement_digest("results_are_honest");
    let definition = project.statement_digest("walk_exit_status");
    let example = project.example_digest("open_exits_one");
    assert_eq!(
        project.log_source(),
        format!(
            r#"approval {{
  item: results_are_honest
  kind: intent
  action: approve
  by: "owner"
  digest: "{intent}"
  digest_version: 1
  text: "LemmaSpec \"never\" reports\n\t\\ an artifact as passing while one of its claims is open."
}}
approval {{
  item: open_exits_one
  kind: example
  action: approve
  by: "owner"
  digest: "{example}"
  digest_version: 1
  links: [walk_exit_status: "{definition}"]
  channels: [cli, library]
}}
"#
        )
    );
    assert_eq!(
        project.log().entries[0].text.as_deref(),
        Some("LemmaSpec \"never\" reports\n\t\\ an artifact as passing while one of its claims is open.")
    );
}

// Covers AE13 (data half)
#[test]
fn only_the_latest_approval_counts_so_text_reverted_to_an_earlier_approval_is_unapproved() {
    let project = Project::new("ae13");
    project.approve(&["walk_exit_status"]).unwrap();
    let text_a = project.statement_digest("walk_exit_status");
    project.edit(CATALOG, DEFINITION_TEXT, CHANGED_TEXT);
    project.approve(&["walk_exit_status"]).unwrap();
    let text_b = project.statement_digest("walk_exit_status");
    project.edit(CATALOG, CHANGED_TEXT, DEFINITION_TEXT);

    let log = project.log();
    let latest = log.latest()["walk_exit_status"];
    assert_eq!(latest.digest, text_b);
    assert_eq!(latest.text.as_deref(), Some(CHANGED_TEXT));
    assert_eq!(project.statement_digest("walk_exit_status"), text_a);
    assert_ne!(latest.digest, text_a);
    assert_eq!(
        project.reconfirm(&["walk_exit_status"]).unwrap_err(),
        "walk_exit_status changed since its latest approval; approve it instead"
    );
}

#[test]
fn reconfirming_an_unchanged_example_records_its_definitions_new_digest() {
    let project = Project::new("reconfirm");
    project
        .approve(&["walk_exit_status", "open_exits_one"])
        .unwrap();
    let before = project.statement_digest("walk_exit_status");
    project.edit(CATALOG, DEFINITION_TEXT, CHANGED_TEXT);
    let after = project.statement_digest("walk_exit_status");
    assert_ne!(before, after);

    project.reconfirm(&["open_exits_one"]).unwrap();
    let log = project.log();
    let [_, approved, reconfirmed] = log.entries.as_slice() else {
        panic!("{log:?}")
    };
    assert_eq!(approved.links, links(&[("walk_exit_status", &before)]));
    assert_eq!(
        reconfirmed,
        &Entry {
            links: links(&[("walk_exit_status", &after)]),
            ..approved.clone()
        }
    );
}

#[test]
fn reconfirm_refuses_unknown_never_approved_and_changed_items_and_writes_nothing() {
    let project = Project::new("reconfirm-refused");
    project.approve(&["walk_exit_status"]).unwrap();
    project.edit(CATALOG, DEFINITION_TEXT, CHANGED_TEXT);
    let before = project.log_source();
    for (ids, error) in [
        (["no_such_item"], "unknown item no_such_item"),
        (
            ["invalid_exits_two"],
            "invalid_exits_two is not approved; approve it instead",
        ),
        (
            ["walk_exit_status"],
            "walk_exit_status changed since its latest approval; approve it instead",
        ),
    ] {
        assert_eq!(project.reconfirm(&ids).unwrap_err(), error);
        assert_eq!(project.log_source(), before);
    }
}

#[test]
fn approved_items_deleted_since_are_reinstated_from_their_latest_entry_until_removed() {
    let project = Project::new("reinstate");
    project
        .approve(&[
            "walk_exit_status",
            "library_returns_report",
            "open_exits_one",
            "invalid_exits_two",
        ])
        .unwrap();
    project.edit(TESTS, OPEN_EXITS_ONE, "");
    project.edit(CATALOG, EXCEPTION, "");
    let (catalog, examples) = (project.catalog(), project.examples());

    let log = project.log();
    let reinstated = log.reinstated(&catalog, &examples);
    assert_eq!(reinstated, [&log.entries[1], &log.entries[2]]);
    let [exception, example] = reinstated.as_slice() else {
        unreachable!()
    };
    assert_eq!(exception.kind, ItemKind::Exception);
    assert_eq!(
        exception.text.as_deref(),
        Some(
            "The library returns the report value, and its status field carries the same outcomes."
        )
    );
    assert_eq!(example.kind, ItemKind::Example);
    assert_eq!(
        example.links.keys().collect::<Vec<_>>(),
        ["walk_exit_status"]
    );
    assert_eq!(example.channels, ["cli", "library"]);

    project
        .remove(&["library_returns_report", "open_exits_one"])
        .unwrap();
    assert!(project.log().reinstated(&catalog, &examples).is_empty());
}

#[test]
fn remove_is_refused_while_approved_items_link_to_the_target_and_names_them() {
    let project = Project::new("remove");
    project
        .approve(&[
            "results_are_honest",
            "walk_exit_status",
            "library_returns_report",
            "open_exits_one",
        ])
        .unwrap();
    let approved = project.log().latest()["walk_exit_status"].clone();
    let before = project.log_source();
    for (ids, error) in [
        (
            ["walk_exit_status"],
            "cannot remove walk_exit_status: approved items still link to it: library_returns_report, open_exits_one",
        ),
        (
            ["results_are_honest"],
            "cannot remove results_are_honest: approved items still link to it: walk_exit_status",
        ),
        (["no_such_item"], "unknown item no_such_item"),
        (
            ["invalid_exits_two"],
            "invalid_exits_two is not approved, so there is nothing to remove",
        ),
    ] {
        assert_eq!(project.remove(&ids).unwrap_err(), error);
        assert_eq!(project.log_source(), before);
    }

    // A deleted example that is still approved still links to its definition.
    project.edit(CATALOG, EXCEPTION, "");
    project.edit(TESTS, OPEN_EXITS_ONE, "");
    project.remove(&["library_returns_report"]).unwrap();
    assert_eq!(
        project.remove(&["walk_exit_status"]).unwrap_err(),
        "cannot remove walk_exit_status: approved items still link to it: open_exits_one"
    );

    // Removing the dependent in the same command clears the way.
    project.edit(CATALOG, DEFINITION, "");
    project
        .remove(&["walk_exit_status", "open_exits_one"])
        .unwrap();
    let log = project.log();
    assert_eq!(
        log.latest()["walk_exit_status"],
        &Entry {
            action: Action::Remove,
            links: BTreeMap::new(),
            text: None,
            ..approved
        }
    );
    assert_eq!(log.latest()["open_exits_one"].action, Action::Remove);
}

#[test]
fn approving_an_unknown_id_or_while_the_catalog_has_a_link_error_writes_nothing() {
    let project = Project::new("refused");
    project.approve(&["results_are_honest"]).unwrap();
    let before = project.log_source();
    assert_eq!(
        project
            .approve(&["walk_exit_status", "no_such_item"])
            .unwrap_err(),
        "unknown item no_such_item"
    );
    assert_eq!(project.log_source(), before);

    project.edit(
        CATALOG,
        "refines: results_are_honest",
        "refines: results_are_honst",
    );
    for refused in [
        project.approve(&["results_are_honest"]),
        project.reconfirm(&["results_are_honest"]),
        project.remove(&["results_are_honest"]),
    ] {
        let error = refused.unwrap_err();
        assert!(
            error.contains("unknown reference results_are_honst"),
            "{error}"
        );
        assert_eq!(project.log_source(), before);
    }
}

#[test]
fn approving_needs_an_approver_and_an_id_that_names_one_item() {
    let project = Project::new("ambiguous");
    assert_eq!(
        project
            .append(|_, catalog, examples| {
                approvals::approve(catalog, examples, &["walk_exit_status"], " ")
            })
            .unwrap_err(),
        "an approval needs the approver's name"
    );
    project.write(
        "tests/clash.rs",
        "/// intent: example walk_exit_status exemplifies walk_exit_status on cli\n#[test]\nfn clash() {}\n",
    );
    assert_eq!(
        project.approve(&["walk_exit_status"]).unwrap_err(),
        "walk_exit_status names both a statement and an example"
    );
    assert_eq!(project.log_source(), "");
}

#[test]
fn every_write_leaves_the_existing_log_bytes_as_an_unchanged_prefix() {
    let project = Project::new("prefix");
    // Kept by hand, ending in a comment with no final newline.
    project.write(
        &format!("intent/{APPROVALS_FILE}"),
        "// The approval log.\n// Entries follow",
    );
    let mut written = project.log_source();
    let mut grows = |result: Result<(), String>| {
        result.unwrap();
        let now = project.log_source();
        assert!(
            now.len() > written.len() && now.starts_with(&written),
            "{now}"
        );
        written = now;
    };
    grows(project.approve(&["walk_exit_status", "open_exits_one"]));
    project.edit(CATALOG, DEFINITION_TEXT, CHANGED_TEXT);
    grows(project.reconfirm(&["open_exits_one"]));
    grows(project.approve(&["walk_exit_status"]));
    grows(project.remove(&["open_exits_one"]));
    assert_eq!(project.log().entries.len(), 5);
}

#[test]
fn entries_appended_on_two_branches_load_in_either_merge_order_and_the_later_one_wins() {
    let project = Project::new("branches");
    project.approve(&["results_are_honest"]).unwrap();
    let base = project.log_source();
    let ours = approvals::approve(
        &project.catalog(),
        &project.examples(),
        &["walk_exit_status", "open_exits_one"],
        "alice",
    )
    .unwrap();
    project.edit(CATALOG, DEFINITION_TEXT, CHANGED_TEXT);
    let theirs = approvals::approve(
        &project.catalog(),
        &project.examples(),
        &["walk_exit_status"],
        "bob",
    )
    .unwrap();

    for (first, second, winner, text) in [
        (&ours, &theirs, "bob", CHANGED_TEXT),
        (&theirs, &ours, "alice", DEFINITION_TEXT),
    ] {
        let log = Log::parse(&format!("{base}{first}{second}")).unwrap();
        assert_eq!(log.entries.len(), 4);
        let latest = log.latest();
        assert_eq!(latest.len(), 3);
        assert_eq!(latest["walk_exit_status"].by, winner);
        assert_eq!(latest["walk_exit_status"].text.as_deref(), Some(text));
        assert_eq!(latest["open_exits_one"].by, "alice");
        assert_eq!(latest["results_are_honest"].by, "owner");
    }
}

#[test]
fn approving_a_changed_item_again_appends_an_entry_that_differs_only_where_it_changed() {
    let project = Project::new("twice");
    project.approve(&["walk_exit_status"]).unwrap();
    project.edit(CATALOG, DEFINITION_TEXT, CHANGED_TEXT);
    project.approve(&["walk_exit_status"]).unwrap();

    let source = project.log_source();
    let lines: Vec<&str> = source.lines().collect();
    let (first, second) = lines.split_at(lines.len() / 2);
    let changed: Vec<_> = first
        .iter()
        .zip(second)
        .filter(|(a, b)| a != b)
        .map(|(a, _)| a.split(':').next().unwrap().trim())
        .collect();
    assert_eq!(changed, ["digest", "text"], "{source}");

    let log = project.log();
    let [first, second] = log.entries.as_slice() else {
        panic!("{log:?}")
    };
    assert_eq!(second.text.as_deref(), Some(CHANGED_TEXT));
    assert_eq!(
        first,
        &Entry {
            digest: first.digest.clone(),
            text: first.text.clone(),
            ..second.clone()
        }
    );
}

#[test]
fn a_missing_log_is_empty_and_malformed_entries_are_rejected_with_their_line() {
    assert_eq!(Log::read(&BTreeMap::new()).unwrap(), Log::default());
    let entry = "approval {\n  item: x\n  kind: intent\n  action: approve\n  by: \"owner\"\n  digest: \"sha256:a\"\n  digest_version: 1\n}\n";
    assert_eq!(Log::parse(entry).unwrap().entries.len(), 1);
    for (source, error) in [
        (
            entry.replace("kind: intent", "kind: statement"),
            "approvals.intent:3: approval of x has unknown kind statement; expected intent, definition, exception, channel or example",
        ),
        (
            entry.replace("action: approve", "action: revoke"),
            "approvals.intent:4: approval of x has unknown action revoke; expected approve or remove",
        ),
        (
            entry.replace("  digest: \"sha256:a\"\n", ""),
            "approvals.intent:1: approval of x needs digest",
        ),
        (
            entry.replace("  item: x\n", ""),
            "approvals.intent:1: approval needs item",
        ),
        (
            entry.replace("  by:", "  approver: \"owner\"\n  by:"),
            "approvals.intent:5: approval of x has no field approver",
        ),
        (
            entry.replace("  by:", "  by: \"owner\"\n  by:"),
            "approvals.intent:6: duplicate field by in approval of x",
        ),
        (
            entry.replace("digest_version: 1", "digest_version: \"1\""),
            "approvals.intent:7: digest_version of x must be an integer",
        ),
        (
            entry.replace("  digest_version: 1\n", "  digest_version: 1\n  links: [y]\n"),
            "approvals.intent:8: links of x must be a list of target: \"digest\" entries",
        ),
        (
            entry.replace(
                "  digest_version: 1\n",
                "  digest_version: 1\n  links: [y: \"sha256:b\", y: \"sha256:c\"]\n",
            ),
            "approvals.intent:8: links of x name y twice",
        ),
        (
            entry.replace("  digest_version: 1\n", "  digest_version: 1\n  channels: [\"cli\"]\n"),
            "approvals.intent:8: channels of x must be a list of channel IDs",
        ),
        (
            format!("{entry}run {{\n}}\n"),
            "approvals.intent:9: expected an approval entry, found run",
        ),
        (
            "catalog log {\n}\n".to_string(),
            "approvals.intent:1: expected approval entries, found catalog log",
        ),
    ] {
        assert_eq!(Log::parse(&source).unwrap_err(), error, "{source}");
    }
}
