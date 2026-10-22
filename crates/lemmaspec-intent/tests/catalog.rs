use std::{cell::RefCell, collections::BTreeMap, io};

use lemmaspec_intent::{
    loader::{load, Reader},
    syntax::{parse, Value},
    ExceptionKind, Statement, StatementKind,
};

const ROOT: &str = r#"catalog lemmaspec {
  use "./channels.intent" as channels
  use "./walk.intent" as walk
}
"#;

const CHANNELS: &str = r#"catalog channels {
  channel cli {
    text: "The lemmaspec command line."
  }
  channel library {
    text: "The lemmaspec Rust library."
  }
}
"#;

const WALK: &str = r#"catalog walk {
  use "./channels.intent" as ch

  intent results_are_honest {
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
}
"#;

const APPROVALS: &str = r#"approval { item: walk_exit_status action: approve by: "owner"
           digest: "sha256:abc" digest_version: 1
           links: [results_are_honest: "sha256:def"] text: "walk exits 0 when every expectation holds." }
"#;

const RUNS: &str = r#"run { example: walk_open_expectation_exits_one channel: cli outcome: fail
      digest: "sha256:abc" digest_version: 1 source: "sha256:def" base: "3bf8c7d" }
"#;

fn files(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(path, source)| (path.to_string(), source.to_string()))
        .collect()
}

fn base() -> BTreeMap<String, String> {
    files(&[
        ("catalog.intent", ROOT),
        ("channels.intent", CHANNELS),
        ("walk.intent", WALK),
    ])
}

fn with(path: &str, source: &str) -> BTreeMap<String, String> {
    let mut files = base();
    files.insert(path.into(), source.into());
    files
}

fn error(files: BTreeMap<String, String>) -> String {
    load(&files).unwrap_err()
}

struct CountingReader {
    files: BTreeMap<String, String>,
    reads: RefCell<Vec<String>>,
}

impl Reader for CountingReader {
    fn read(&self, path: &str) -> io::Result<String> {
        self.reads.borrow_mut().push(path.into());
        self.files.read(path)
    }

    fn list(&self) -> io::Result<Vec<String>> {
        self.files.list()
    }
}

#[test]
fn root_importing_two_files_loads_every_statement_with_kind_prose_and_links() {
    let catalog = load(&base()).unwrap();
    assert_eq!(
        catalog
            .statements
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "cli",
            "library",
            "library_returns_report",
            "results_are_honest",
            "walk_exit_status"
        ]
    );
    assert_eq!(
        catalog.statements["results_are_honest"],
        Statement {
            id: "results_are_honest".into(),
            text: "LemmaSpec never reports an artifact as passing while one of its claims is open."
                .into(),
            file: "walk.intent".into(),
            kind: StatementKind::Intent,
        }
    );
    assert_eq!(
        catalog.statements["walk_exit_status"].kind,
        StatementKind::Definition {
            refines: "results_are_honest".into()
        }
    );
    assert_eq!(
        catalog.statements["library_returns_report"],
        Statement {
            id: "library_returns_report".into(),
            text: "The library returns the report value, and its status field carries the same outcomes."
                .into(),
            file: "walk.intent".into(),
            kind: StatementKind::Exception {
                narrows: "walk_exit_status".into(),
                kind: ExceptionKind::HoldsInstead,
                channels: vec!["library".into()],
            },
        }
    );
    assert_eq!(
        catalog.statements["library"],
        Statement {
            id: "library".into(),
            text: "The lemmaspec Rust library.".into(),
            file: "channels.intent".into(),
            kind: StatementKind::Channel,
        }
    );
}

// Covers AE16 (loading half): moving a statement into a newly imported file
// changes only its file of origin.
#[test]
fn statement_moved_into_a_newly_imported_file_loads_unchanged_apart_from_its_file() {
    let before = load(&base()).unwrap();
    let mut moved = with(
        "catalog.intent",
        &ROOT.replace("}\n", "  use \"./exceptions.intent\" as exceptions\n}\n"),
    );
    let walk_only = WALK.replace(
        &WALK[WALK.find("  exception").unwrap()..WALK.rfind('}').unwrap()],
        "",
    );
    moved.insert("walk.intent".into(), walk_only);
    moved.insert(
        "exceptions.intent".into(),
        r#"catalog exceptions {
  use "./walk.intent" as w
  use "./channels.intent" as ch

  exception library_returns_report {
    narrows: w.walk_exit_status
    channels: [ch.library]
    kind: holds_instead
    text: "The library returns the report value, and its status field carries the same outcomes."
  }
}
"#
        .into(),
    );
    let after = load(&moved).unwrap();
    assert_eq!(
        after.statements["library_returns_report"].file,
        "exceptions.intent"
    );
    let without_files = |catalog: lemmaspec_intent::Catalog| {
        catalog
            .statements
            .into_values()
            .map(|s| (s.id, s.text, s.kind))
            .collect::<Vec<_>>()
    };
    assert_eq!(without_files(before), without_files(after));
}

// Covers AE6.
#[test]
fn exception_narrowing_an_unknown_definition_names_the_reference_and_its_file() {
    assert_eq!(
        error(with(
            "walk.intent",
            &WALK.replace("narrows: walk_exit_status", "narrows: walk_exit_state")
        )),
        "walk.intent:12: unknown reference walk_exit_state in narrows of library_returns_report"
    );
}

#[test]
fn missing_files_are_reported_at_the_importing_use() {
    let mut missing_import = base();
    missing_import.remove("walk.intent");
    assert_eq!(
        error(missing_import),
        "catalog.intent:3: imported file walk.intent not found"
    );
    assert_eq!(
        error(files(&[])),
        "catalog.intent not found in the catalog directory"
    );
}

#[test]
fn duplicate_id_across_files_names_both_files() {
    assert_eq!(
        error(files(&[
            (
                "catalog.intent",
                "catalog root {\n  use \"./a.intent\" as a\n  use \"./b.intent\" as b\n}\n"
            ),
            (
                "a.intent",
                "catalog a {\n  intent shared { text: \"From a.\" }\n}\n"
            ),
            (
                "b.intent",
                "catalog b {\n  intent shared { text: \"From b.\" }\n}\n"
            ),
        ])),
        "duplicate id shared in a.intent:2 and b.intent:2"
    );
}

#[test]
fn import_cycle_fails_and_a_file_imported_along_two_paths_loads_once() {
    assert_eq!(
        error(files(&[
            (
                "catalog.intent",
                "catalog root {\n  use \"./a.intent\" as a\n}\n"
            ),
            ("a.intent", "catalog a {\n  use \"./b.intent\" as b\n}\n"),
            ("b.intent", "catalog b {\n  use \"./a.intent\" as a\n}\n"),
        ])),
        "b.intent:2: import cycle through a.intent"
    );
    assert_eq!(
        error(files(&[(
            "catalog.intent",
            "catalog root {\n  use \"./catalog.intent\" as me\n}\n"
        )])),
        "catalog.intent:2: import cycle through catalog.intent"
    );

    let reader = CountingReader {
        files: files(&[
            (
                "catalog.intent",
                "catalog root {\n  use \"./a.intent\" as a\n  use \"./b.intent\" as b\n}\n",
            ),
            (
                "a.intent",
                "catalog a {\n  use \"./shared.intent\" as s\n  definition term_a { refines: s.goal text: \"A.\" }\n}\n",
            ),
            (
                "b.intent",
                "catalog b {\n  use \"./shared.intent\" as s\n  definition term_b { refines: s.goal text: \"B.\" }\n}\n",
            ),
            (
                "shared.intent",
                "catalog shared {\n  intent goal { text: \"Goal.\" }\n}\n",
            ),
        ]),
        reads: RefCell::default(),
    };
    let catalog = load(&reader).unwrap();
    assert_eq!(
        reader.reads.into_inner(),
        ["catalog.intent", "a.intent", "shared.intent", "b.intent"]
    );
    for term in ["term_a", "term_b"] {
        assert_eq!(
            catalog.statements[term].kind,
            StatementKind::Definition {
                refines: "goal".into()
            }
        );
    }
}

#[test]
fn links_between_the_wrong_kinds_of_statement_fail() {
    assert_eq!(
        error(with(
            "walk.intent",
            &WALK.replace("refines: results_are_honest", "refines: ch.cli")
        )),
        "walk.intent:8: refines of walk_exit_status must name an intent, but ch.cli is a channel"
    );
    assert_eq!(
        error(with(
            "walk.intent",
            &WALK.replace("narrows: walk_exit_status", "narrows: results_are_honest")
        )),
        "walk.intent:12: narrows of library_returns_report must name a definition, but results_are_honest is an intent"
    );
}

#[test]
fn import_path_that_leaves_the_catalog_directory_fails() {
    for path in [
        "../outside.intent",
        "./sub/../../outside.intent",
        "/outside.intent",
        "C:\\\\outside.intent",
    ] {
        let mut files = with(
            "catalog.intent",
            &format!("catalog root {{\n  use \"{path}\" as outside\n}}\n"),
        );
        files.insert(
            "../outside.intent".into(),
            "catalog outside {\n  intent escaped { text: \"Outside.\" }\n}\n".into(),
        );
        let error = error(files);
        assert!(
            error.starts_with("catalog.intent:2: import ")
                && error.ends_with(" must be a relative path inside the catalog directory"),
            "{error}"
        );
    }
}

#[test]
fn intent_file_the_root_does_not_reach_fails() {
    assert_eq!(
        error(with(
            "stray.intent",
            "catalog stray {\n  intent orphan { text: \"Nobody imports this.\" }\n}\n"
        )),
        "stray.intent: not imported from catalog.intent"
    );
    assert_eq!(
        error(with("sub/approvals.intent", APPROVALS)),
        "sub/approvals.intent: not imported from catalog.intent"
    );
}

#[test]
fn unimported_approvals_and_runs_files_pass() {
    let mut files = base();
    files.insert("approvals.intent".into(), APPROVALS.into());
    files.insert("runs.intent".into(), RUNS.into());
    files.insert("README.md".into(), "# Not a catalog file\n".into());
    assert_eq!(load(&files).unwrap(), load(&base()).unwrap());
}

#[test]
fn bare_id_resolves_only_within_its_own_file() {
    let root = "catalog root {\n  use \"./a.intent\" as a\n  use \"./b.intent\" as b\n}\n";
    let goal = "catalog a {\n  intent goal { text: \"A goal.\" }\n}\n";
    for (term, reference) in [
        (
            "catalog b {\n  definition term {\n    refines: goal\n    text: \"A term.\"\n  }\n}\n",
            "goal",
        ),
        (
            "catalog b {\n  use \"./a.intent\" as a\n  definition term {\n    refines: goal\n    text: \"A term.\"\n  }\n}\n",
            "goal",
        ),
        (
            "catalog b {\n  definition term {\n    refines: x.goal\n    text: \"A term.\"\n  }\n}\n",
            "x.goal",
        ),
    ] {
        let line = term.lines().position(|l| l.contains("refines")).unwrap() + 1;
        assert_eq!(
            error(files(&[
                ("catalog.intent", root),
                ("a.intent", goal),
                ("b.intent", term)
            ])),
            format!("b.intent:{line}: unknown reference {reference} in refines of term")
        );
    }
}

#[test]
fn exception_kind_and_channels_are_checked() {
    assert_eq!(
        error(with(
            "walk.intent",
            &WALK.replace("    kind: holds_instead\n", "")
        )),
        "walk.intent:11: exception library_returns_report needs kind"
    );
    assert_eq!(
        error(with(
            "walk.intent",
            &WALK.replace("kind: holds_instead", "kind: maybe")
        )),
        "walk.intent:14: exception library_returns_report has unknown kind maybe; expected does_not_apply or holds_instead"
    );
    assert_eq!(
        error(with(
            "walk.intent",
            &WALK.replace("channels: [ch.library]", "channels: [walk_exit_status]")
        )),
        "walk.intent:13: channels of library_returns_report must name a channel, but walk_exit_status is a definition"
    );
    let does_not_apply = load(&with(
        "walk.intent",
        &WALK.replace("kind: holds_instead", "kind: does_not_apply"),
    ))
    .unwrap();
    assert!(matches!(
        does_not_apply.statements["library_returns_report"].kind,
        StatementKind::Exception {
            kind: ExceptionKind::DoesNotApply,
            ..
        }
    ));
}

#[test]
fn statement_without_prose_fails() {
    assert_eq!(
        error(with(
            "channels.intent",
            &CHANNELS.replace("    text: \"The lemmaspec command line.\"\n", "")
        )),
        "channels.intent:2: channel cli needs text"
    );
    assert_eq!(
        error(with(
            "channels.intent",
            &CHANNELS.replace("\"The lemmaspec command line.\"", "\"  \"")
        )),
        "channels.intent:3: text of cli must be a nonempty string"
    );
}

#[test]
fn backslash_import_paths_load_the_same_files_as_forward_slashes() {
    let tree = |root_imports: &str, walk_import: &str| {
        let mut files = files(&[("channels.intent", CHANNELS)]);
        files.insert(
            "catalog.intent".into(),
            format!("catalog root {{\n{root_imports}}}\n"),
        );
        files.insert(
            "sub/walk.intent".into(),
            WALK.replace("./channels.intent", walk_import),
        );
        files
    };
    let forward = load(&tree(
        "  use \"./channels.intent\" as channels\n  use \"./sub/walk.intent\" as walk\n",
        "../channels.intent",
    ))
    .unwrap();
    let backslash = load(&tree(
        r#"  use ".\\channels.intent" as channels
  use ".\\sub\\walk.intent" as walk
"#,
        r"..\\channels.intent",
    ))
    .unwrap();
    assert_eq!(forward, backslash);
    assert_eq!(
        forward.statements["walk_exit_status"].file,
        "sub/walk.intent"
    );
}

#[test]
fn malformed_statements_and_imports_fail() {
    let shapes = "  intent i { text: \"I.\" }\n  definition d { refines: i text: \"D.\" }\n  channel c { text: \"C.\" }\n";
    let exception = |channels: &str| {
        format!("catalog root {{\n{shapes}  exception e {{ narrows: d channels: {channels} kind: does_not_apply text: \"E.\" }}\n}}\n")
    };
    let empty = [
        ("a.intent", "catalog a {\n}\n"),
        ("b.intent", "catalog b {\n}\n"),
    ];
    let header = [("header.intent", "intent x { text: \"X.\" }\n")];
    for (source, extra, expected) in [
        (
            "catalog root {\n  goal g { text: \"G.\" }\n}\n".to_string(),
            &[][..],
            "catalog.intent:2: unknown statement kind goal",
        ),
        (
            "catalog root {\n  intent { text: \"G.\" }\n}\n".to_string(),
            &[],
            "catalog.intent:2: intent needs an id",
        ),
        (
            "catalog root {\n  intent g {\n    text: \"G.\"\n    refines: g\n  }\n}\n".to_string(),
            &[],
            "catalog.intent:4: intent g has no field refines",
        ),
        (
            "catalog root {\n  intent g {\n    text: \"G.\"\n    text: \"Again.\"\n  }\n}\n"
                .to_string(),
            &[],
            "catalog.intent:4: duplicate field text in g",
        ),
        (
            "catalog root {\n  definition d { text: \"D.\" }\n}\n".to_string(),
            &[],
            "catalog.intent:2: definition d needs refines",
        ),
        (
            "catalog root {\n  intent i { text: 5 }\n}\n".to_string(),
            &[],
            "catalog.intent:2: text of i must be a nonempty string",
        ),
        (
            "catalog root {\n  intent i { text: \"I.\" }\n  definition d { refines: \"i\" text: \"D.\" }\n}\n"
                .to_string(),
            &[],
            "catalog.intent:3: refines of d must be a statement reference",
        ),
        (
            exception("[]"),
            &[],
            "catalog.intent:5: exception e names no channels",
        ),
        (
            exception("[c, c]"),
            &[],
            "catalog.intent:5: exception e names channel c twice",
        ),
        (
            exception("c"),
            &[],
            "catalog.intent:5: channels of e must be a list",
        ),
        (
            "catalog root {\n  use \"./a.intent\" as a\n  use \"./b.intent\" as a\n}\n".to_string(),
            &empty,
            "catalog.intent:3: duplicate import alias a",
        ),
        (
            "catalog root {\n  use \"./header.intent\" as h\n}\n".to_string(),
            &header,
            "header.intent:1: a catalog file must start with catalog NAME",
        ),
    ] {
        let mut files = files(extra);
        files.insert("catalog.intent".into(), source.clone());
        assert_eq!(error(files), expected, "{source}");
    }
}

#[test]
fn parser_reads_approval_and_run_entries_as_field_blocks() {
    let approvals = parse(APPROVALS, "approvals.intent").unwrap();
    assert_eq!(approvals.catalog, None);
    assert!(approvals.imports.is_empty());
    let block = &approvals.blocks[0];
    assert_eq!(
        (block.kind.as_str(), block.name.as_deref(), block.line),
        ("approval", None, 1)
    );
    let fields: Vec<_> = block
        .fields
        .iter()
        .map(|f| (f.name.as_str(), f.value.clone(), f.line))
        .collect();
    assert_eq!(
        fields,
        [
            ("item", Value::Ident("walk_exit_status".into()), 1),
            ("action", Value::Ident("approve".into()), 1),
            ("by", Value::Str("owner".into()), 1),
            ("digest", Value::Str("sha256:abc".into()), 2),
            ("digest_version", Value::Int(1), 2),
            (
                "links",
                Value::List(vec![Value::Entry(
                    "results_are_honest".into(),
                    Box::new(Value::Str("sha256:def".into()))
                )]),
                3
            ),
            (
                "text",
                Value::Str("walk exits 0 when every expectation holds.".into()),
                3
            ),
        ]
    );
    let runs = parse(RUNS, "runs.intent").unwrap();
    assert_eq!(runs.blocks.len(), 1);
    assert_eq!(runs.blocks[0].kind, "run");
    assert_eq!(
        runs.blocks[0]
            .fields
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        [
            "example",
            "channel",
            "outcome",
            "digest",
            "digest_version",
            "source",
            "base"
        ]
    );

    let walk = parse(WALK, "walk.intent").unwrap();
    assert_eq!(walk.catalog.as_deref(), Some("walk"));
    assert_eq!(
        (
            walk.imports[0].path.as_str(),
            walk.imports[0].alias.as_str(),
            walk.imports[0].line
        ),
        ("./channels.intent", "ch", 2)
    );
    let channels = &walk.blocks[2].fields[1];
    assert_eq!(
        (channels.name.as_str(), &channels.value, channels.line),
        (
            "channels",
            &Value::List(vec![Value::Qualified("ch".into(), "library".into())]),
            13
        )
    );
    assert_eq!(
        parse("run { tags: [a, b.c, \"d \\\"e\\\"\", 7,] }", "x.intent")
            .unwrap()
            .blocks[0]
            .fields[0]
            .value,
        Value::List(vec![
            Value::Ident("a".into()),
            Value::Qualified("b".into(), "c".into()),
            Value::Str("d \"e\"".into()),
            Value::Int(7),
        ])
    );
}

#[test]
fn syntax_errors_name_the_file_and_line() {
    for (source, expected) in [
        (
            "catalog walk {\n  intent x {\n    text: \"unterminated\n  }\n}\n",
            "walk.intent:3: unterminated string",
        ),
        (
            "catalog walk {\n  intent x {\n    text \"missing colon\"\n  }\n}\n",
            "walk.intent:3: expected ':', found a string",
        ),
        (
            "catalog walk {\n  intent x { text: \"X.\" }\n  use \"./a.intent\" as a\n}\n",
            "walk.intent:3: use must come before statements",
        ),
        (
            "catalog walk {\n  intent x { text: \"X.\" }\n}\napproval { item: x }\n",
            "walk.intent:4: expected end of file, found approval",
        ),
    ] {
        assert_eq!(parse(source, "walk.intent").unwrap_err(), expected);
    }
    assert_eq!(
        error(with("walk.intent", "catalog walk {\n  intent x {\n")),
        "walk.intent:2: expected field name, found end of file"
    );
}
