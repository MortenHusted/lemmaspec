use std::{collections::BTreeMap, fs, path::Path};

use lemmaspec_intent::{
    digest::example_digest,
    examples::{scan, Example, ExampleIssue, Scan},
    loader::load,
    Catalog,
};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/examples");

const CATALOG: &str = r#"catalog fixture {
  intent results_are_honest {
    text: "LemmaSpec never reports an artifact as passing while one of its claims is open."
  }
  definition walk_exit_status {
    refines: results_are_honest
    text: "walk exits 0 when every expectation holds, 1 when one is open, and 2 when the artifact is invalid."
  }
  exception library_returns_report {
    narrows: walk_exit_status
    channels: [library]
    kind: holds_instead
    text: "The library returns the report value, and its status field carries the same outcomes."
  }
  exception no_web_exit_status {
    narrows: walk_exit_status
    channels: [web]
    kind: does_not_apply
    text: "The web view has no exit status."
  }
  channel cli {
    text: "The lemmaspec command line."
  }
  channel library {
    text: "The lemmaspec Rust library."
  }
  channel web {
    text: "The lemmaspec web view."
  }
}
"#;

fn catalog() -> Catalog {
    load(&BTreeMap::from([(
        "catalog.intent".to_string(),
        CATALOG.to_string(),
    )]))
    .unwrap()
}

fn fixture(name: &str) -> Result<Scan, String> {
    scan(&Path::new(FIXTURES).join(name), &catalog())
}

/// Scan a directory that holds `source` as its only test file, `example.rs`.
fn scan_source(label: &str, source: &str) -> Scan {
    let dir = std::env::temp_dir().join(format!("lemmaspec-intent-{label}-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("example.rs"), source).unwrap();
    let scanned = scan(&dir, &catalog());
    fs::remove_dir_all(&dir).unwrap();
    scanned.unwrap()
}

fn digest(label: &str, source: &str) -> String {
    let scanned = scan_source(label, source);
    assert_eq!(scanned.examples.len(), 1, "{scanned:?}");
    scanned.examples[0].digest.clone()
}

fn example(
    id: &str,
    statement: &str,
    channels: &[&str],
    file: &str,
    test: &str,
    line: usize,
) -> Example {
    Example {
        id: id.into(),
        statement: statement.into(),
        channels: channels.iter().map(|channel| channel.to_string()).collect(),
        file: file.into(),
        test: test.into(),
        line,
        digest: String::new(),
    }
}

fn issue(example: Option<&str>, test: &str, line: usize, reason: &str) -> ExampleIssue {
    ExampleIssue {
        example: example.map(Into::into),
        file: "walk".into(),
        test: test.into(),
        line,
        reason: reason.into(),
    }
}

fn without_digests(mut examples: Vec<Example>) -> Vec<Example> {
    for example in &mut examples {
        assert!(
            example.digest.starts_with("sha256:") && example.digest.len() == 71,
            "{example:?}"
        );
        example.digest.clear();
    }
    examples
}

#[test]
fn tagged_tests_yield_examples_with_sorted_channels_file_stem_and_test_path() {
    let scanned = fixture("tagged").unwrap();
    assert_eq!(
        without_digests(scanned.examples),
        [
            example(
                "cli_open_exits_one",
                "walk_exit_status",
                &["cli"],
                "other",
                "cli_open_exits_one",
                1
            ),
            example(
                "open_exits_one",
                "walk_exit_status",
                &["cli", "library"],
                "walk",
                "open_expectation_exits_one",
                4
            ),
            example(
                "invalid_exits_two",
                "walk_exit_status",
                &["cli"],
                "walk",
                "exit_codes::invalid::invalid_artifact_exits_two",
                18
            ),
            example(
                "library_report",
                "library_returns_report",
                &["library"],
                "walk",
                "library_returns_the_report",
                24
            ),
        ]
    );
}

#[test]
fn malformed_tags_and_statements_no_example_can_exemplify_are_example_issues() {
    let scanned = fixture("tagged").unwrap();
    let rule = "an example exemplifies a definition or a holds_instead exception";
    assert_eq!(
        scanned.issues,
        [
            issue(
                Some("undeclared"),
                "names_an_undeclared_statement",
                28,
                "unknown statement undeclared_statement"
            ),
            issue(
                Some("honest"),
                "names_an_intent",
                32,
                &format!("results_are_honest is an intent; {rule}")
            ),
            issue(
                Some("no_web"),
                "names_a_does_not_apply_exception",
                36,
                &format!("no_web_exit_status is a does_not_apply exception; {rule}")
            ),
            issue(
                None,
                "has_a_malformed_tag",
                40,
                "malformed tag \"intent: example exemplifies walk_exit_status on cli\"; expected intent: example <id> exemplifies <statement> on <channel>[, <channel>...]"
            ),
        ]
    );
}

#[test]
fn every_misshapen_tag_line_is_malformed() {
    for (index, tag) in [
        "intent: example open exemplifies walk_exit_status",
        "intent: example open exemplifies walk_exit_status on",
        "intent: example open exemplifies walk_exit_status on cli,",
        "intent: example open exemplifies walk_exit_status on cli library",
        "intent: example open exemplifies walk_exit_status on cli, cli",
        "intent: example open-exits exemplifies walk_exit_status on cli",
        "intent: example open exemplify walk_exit_status on cli",
        "intent:example open exemplifies walk_exit_status on cli",
        "intent: open exemplifies walk_exit_status on cli",
    ]
    .into_iter()
    .enumerate()
    {
        let scanned = scan_source(
            &format!("malformed-{index}"),
            &format!("/// {tag}\n#[test]\nfn tagged() {{}}\n"),
        );
        assert!(scanned.examples.is_empty(), "{tag}");
        assert_eq!(scanned.issues.len(), 1, "{tag}");
        assert_eq!(scanned.issues[0].example, None, "{tag}");
        assert!(
            scanned.issues[0].reason.starts_with("malformed tag"),
            "{tag}: {}",
            scanned.issues[0].reason
        );
    }
}

// Covers AE19 (scanner half)
#[test]
fn a_tag_naming_an_undeclared_statement_is_an_example_issue_not_an_error() {
    let scanned = scan_source(
        "undeclared",
        "/// intent: example ghost exemplifies no_such_statement on cli\n#[test]\nfn ghost() {}\n",
    );
    assert!(scanned.examples.is_empty());
    assert_eq!(
        scanned.issues,
        [ExampleIssue {
            example: Some("ghost".into()),
            file: "example".into(),
            test: "ghost".into(),
            line: 1,
            reason: "unknown statement no_such_statement".into(),
        }]
    );
}

// R28's channel half: a channel tag must name a channel its statement can apply on.
#[test]
fn a_tag_naming_a_channel_its_statement_cannot_apply_on_is_an_example_issue() {
    for (index, (tag, reason)) in [
        ("walk_exit_status on cli, ghost", "unknown channel ghost"),
        (
            "walk_exit_status on results_are_honest",
            "results_are_honest is an intent, not a channel",
        ),
        (
            "library_returns_report on cli, library",
            "library_returns_report applies only on library, not on cli",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let scanned = scan_source(
            &format!("channel-{index}"),
            &format!("/// intent: example tagged exemplifies {tag}\n#[test]\nfn tagged() {{}}\n"),
        );
        assert!(scanned.examples.is_empty(), "{tag}");
        assert_eq!(
            scanned.issues,
            [ExampleIssue {
                example: Some("tagged".into()),
                file: "example".into(),
                test: "tagged".into(),
                line: 1,
                reason: reason.into(),
            }],
            "{tag}"
        );
    }
}

#[test]
fn two_tests_claiming_one_example_id_fail_naming_both_locations() {
    assert_eq!(
        fixture("duplicate").unwrap_err(),
        "duplicate example id open_exits_one in a.rs:1 and b.rs:2"
    );
}

const EXAMPLE: &str = r#"/// intent: example open_exits_one exemplifies walk_exit_status on cli
#[test]
fn open_exits_one() {
    let status = 1; // the open case
    assert_eq!(status, 1);
}
"#;

const REFORMATTED: &str = r#"/// intent: example open_exits_one exemplifies walk_exit_status on cli
#[ test ] fn open_exits_one( ) { /* reformatted */ let status=1;
        // another comment
    assert_eq!( status , 1 ) ;
}
"#;

#[test]
fn an_example_digest_ignores_formatting_and_comments() {
    assert_eq!(
        digest("reformatted", REFORMATTED),
        digest("original", EXAMPLE)
    );
}

#[test]
fn an_example_digest_changes_with_its_body_tag_or_ignore_attribute() {
    let original = digest("unchanged", EXAMPLE);
    for (label, changed) in [
        (
            "body",
            EXAMPLE.replace("assert_eq!(status, 1)", "assert_eq!(status, 2)"),
        ),
        ("tag", EXAMPLE.replace("on cli", "on cli, library")),
        ("ignore", EXAMPLE.replace("#[test]", "#[test]\n#[ignore]")),
        ("file attribute", format!("#![cfg(any())]\n{EXAMPLE}")),
        (
            "module attribute",
            format!("#[cfg(any())]\nmod m {{\n{EXAMPLE}}}\n"),
        ),
    ] {
        assert_ne!(digest(label, &changed), original, "{label}");
    }
}

const RAW_STRING: &str = r##"/// intent: example raw_report exemplifies walk_exit_status on cli
#[test]
fn raw_report() {
    let expected = r#"line one
line two
"#;
    assert!(expected.ends_with("two\n"));
}
"##;

#[test]
fn lf_and_crlf_copies_of_a_multiline_raw_string_get_the_same_digest() {
    let crlf = RAW_STRING.replace('\n', "\r\n");
    let lf = digest("lf", RAW_STRING);
    assert_eq!(digest("crlf", &crlf), lf);
    assert_eq!(
        lf,
        example_digest(&[], &syn::parse_str(RAW_STRING).unwrap())
    );
}

#[test]
fn an_example_in_a_plain_module_keeps_its_digest() {
    assert_eq!(
        digest("plain module", &format!("mod m {{\n{EXAMPLE}}}\n")),
        digest("top level", EXAMPLE)
    );
}

// A decoy compiled in place of a cfg'd-out example would run under the
// example's `--exact` path.
#[test]
fn a_tagged_test_sharing_its_path_with_another_test_is_an_example_issue() {
    let decoy = format!(
        "#[cfg(any())]\nmod m {{\n{EXAMPLE}}}\nmod m {{\n    #[test]\n    fn open_exits_one() {{}}\n}}\n"
    );
    assert_eq!(
        scan_source("decoy", &decoy),
        Scan {
            examples: vec![],
            issues: vec![ExampleIssue {
                example: Some("open_exits_one".into()),
                file: "example".into(),
                test: "m::open_exits_one".into(),
                line: 3,
                reason: "another test in example.rs has the path m::open_exits_one, so cargo test --exact cannot tell them apart".into(),
            }],
        }
    );
}

#[test]
fn a_missing_tests_directory_has_no_examples() {
    assert_eq!(fixture("no_such_directory"), Ok(Scan::default()));
}
