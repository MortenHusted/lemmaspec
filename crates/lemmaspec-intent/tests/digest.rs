use std::collections::BTreeMap;

use lemmaspec_intent::{
    digest::{
        canonical_example, canonical_statement, example_digest, statement_digest, DIGEST_VERSION,
    },
    loader::load,
    ExceptionKind, Statement, StatementKind,
};

fn statement(kind: StatementKind, text: &str) -> Statement {
    Statement {
        id: "item".into(),
        text: text.into(),
        file: "walk.intent".into(),
        kind,
    }
}

fn definition(refines: &str, text: &str) -> Statement {
    statement(
        StatementKind::Definition {
            refines: refines.into(),
        },
        text,
    )
}

fn exception(narrows: &str, kind: ExceptionKind, channels: &[&str], text: &str) -> Statement {
    statement(
        StatementKind::Exception {
            narrows: narrows.into(),
            kind,
            channels: channels.iter().map(|channel| channel.to_string()).collect(),
        },
        text,
    )
}

#[test]
fn the_canonical_rendering_of_fixed_statements_equals_the_stored_strings() {
    assert_eq!(DIGEST_VERSION, 1);
    let report = exception(
        "walk_exit_status",
        ExceptionKind::HoldsInstead,
        &["library", "cli"],
        "  The library returns\n the report value,\tand its status field  carries the same outcomes. ",
    );
    for (statement, expected) in [
        (
            statement(StatementKind::Intent, "LemmaSpec never  reports\nfalse passes."),
            "intent-digest/1\nstatement: intent\ntext: LemmaSpec never reports false passes.\n",
        ),
        (
            definition("results_are_honest", "walk exits 1 when an expectation is open."),
            "intent-digest/1\nstatement: definition\ntext: walk exits 1 when an expectation is open.\nrefines: results_are_honest\n",
        ),
        (
            report.clone(),
            "intent-digest/1\nstatement: exception\ntext: The library returns the report value, and its status field carries the same outcomes.\nnarrows: walk_exit_status\nkind: holds_instead\nchannels: cli, library\n",
        ),
        (
            exception("walk_exit_status", ExceptionKind::DoesNotApply, &["web"], "No exit status."),
            "intent-digest/1\nstatement: exception\ntext: No exit status.\nnarrows: walk_exit_status\nkind: does_not_apply\nchannels: web\n",
        ),
        (
            statement(StatementKind::Channel, "The lemmaspec command line."),
            "intent-digest/1\nstatement: channel\ntext: The lemmaspec command line.\n",
        ),
    ] {
        assert_eq!(canonical_statement(&statement), expected);
    }
    assert_eq!(
        statement_digest(&report),
        "sha256:7ca35d18efcd5be6c2bafdbc8304d67994a1c488c1a9b50313d5c41c6561cea8"
    );
}

const ROOT: &str = r#"catalog fixture {
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

  // Why walk's exit status matters.
  intent results_are_honest {
    text: "LemmaSpec never reports an artifact as passing while one of its claims is open."
  }
  definition walk_exit_status {
    refines: results_are_honest
    text: "walk exits 0 when every expectation holds, 1 when one is open, and 2 when the artifact is invalid."
  }
  exception library_returns_report {
    narrows: walk_exit_status
    channels: [ch.library, ch.cli]
    kind: holds_instead
    text: "The library returns the report value, and its status field carries the same outcomes."
  }
}
"#;

const REFLOWED_AND_RECOMMENTED: &str = r#"catalog walk {
  use "./channels.intent" as ch

  // The intent behind every exit status.
  // A second comment line.
  intent results_are_honest {
    text: "  LemmaSpec never reports an artifact\nas passing while one of its claims is open.  "
  }
  definition walk_exit_status {
    // Reflowed prose.
    refines: results_are_honest
    text: "walk exits 0 when every expectation holds,\n1 when one is open,\tand 2 when the artifact is invalid."
  }
  exception library_returns_report {
    narrows: walk_exit_status
    channels: [ch.library, ch.cli]
    kind: holds_instead
    text: "The library returns the report value,  and its status field carries the same outcomes."
  }
}
"#;

const RENAMED_ALIAS: &str = r#"catalog walk {
  use "./channels.intent" as surfaces

  intent results_are_honest {
    text: "LemmaSpec never reports an artifact as passing while one of its claims is open."
  }
  definition walk_exit_status {
    refines: results_are_honest
    text: "walk exits 0 when every expectation holds, 1 when one is open, and 2 when the artifact is invalid."
  }
  exception library_returns_report {
    narrows: walk_exit_status
    channels: [surfaces.library, surfaces.cli]
    kind: holds_instead
    text: "The library returns the report value, and its status field carries the same outcomes."
  }
}
"#;

const MOVED_ROOT: &str = r#"catalog fixture {
  use "./channels.intent" as channels
  use "./walk.intent" as walk
  use "./exit/status.intent" as exit
}
"#;

const MOVED_WALK: &str = r#"catalog walk {
  intent results_are_honest {
    text: "LemmaSpec never reports an artifact as passing while one of its claims is open."
  }
}
"#;

const MOVED_EXIT: &str = r#"catalog exit {
  use "../channels.intent" as channel
  use "../walk.intent" as intents

  definition walk_exit_status {
    refines: intents.results_are_honest
    text: "walk exits 0 when every expectation holds, 1 when one is open, and 2 when the artifact is invalid."
  }
  exception library_returns_report {
    narrows: walk_exit_status
    channels: [channel.library, channel.cli]
    kind: holds_instead
    text: "The library returns the report value, and its status field carries the same outcomes."
  }
}
"#;

fn digests(files: &[(&str, &str)]) -> BTreeMap<String, String> {
    let files: BTreeMap<String, String> = files
        .iter()
        .map(|(path, source)| (path.to_string(), source.to_string()))
        .collect();
    let catalog = load(&files).unwrap();
    catalog
        .statements
        .values()
        .map(|statement| (statement.id.clone(), statement_digest(statement)))
        .collect()
}

// Covers AE16 (digest half)
#[test]
fn statement_digests_ignore_reflowed_prose_comments_files_and_alias_spelling() {
    let base = digests(&[
        ("catalog.intent", ROOT),
        ("channels.intent", CHANNELS),
        ("walk.intent", WALK),
    ]);
    assert_eq!(base.len(), 5);
    let moved = [
        ("catalog.intent", MOVED_ROOT),
        ("channels.intent", CHANNELS),
        ("walk.intent", MOVED_WALK),
        ("exit/status.intent", MOVED_EXIT),
    ];
    let moved_files: BTreeMap<String, String> = moved
        .iter()
        .map(|(path, source)| (path.to_string(), source.to_string()))
        .collect();
    assert_eq!(
        load(&moved_files).unwrap().statements["walk_exit_status"].file,
        "exit/status.intent"
    );
    for variant in [
        digests(&[
            ("catalog.intent", ROOT),
            ("channels.intent", CHANNELS),
            ("walk.intent", REFLOWED_AND_RECOMMENTED),
        ]),
        digests(&[
            ("catalog.intent", ROOT),
            ("channels.intent", CHANNELS),
            ("walk.intent", RENAMED_ALIAS),
        ]),
        digests(&moved),
    ] {
        assert_eq!(variant, base);
    }
}

#[test]
fn a_statement_digest_changes_with_its_prose_kind_links_exception_kind_or_channels() {
    let text = "The library returns the report value.";
    let base = exception(
        "walk_exit_status",
        ExceptionKind::HoldsInstead,
        &["library"],
        text,
    );
    for changed in [
        exception(
            "walk_exit_status",
            ExceptionKind::HoldsInstead,
            &["library"],
            "The library returns a report value.",
        ),
        exception(
            "plan_exit_status",
            ExceptionKind::HoldsInstead,
            &["library"],
            text,
        ),
        exception(
            "walk_exit_status",
            ExceptionKind::DoesNotApply,
            &["library"],
            text,
        ),
        exception(
            "walk_exit_status",
            ExceptionKind::HoldsInstead,
            &["cli"],
            text,
        ),
        exception(
            "walk_exit_status",
            ExceptionKind::HoldsInstead,
            &["cli", "library"],
            text,
        ),
        definition("walk_exit_status", text),
    ] {
        assert_ne!(
            statement_digest(&changed),
            statement_digest(&base),
            "{changed:?}"
        );
    }
    assert_ne!(
        statement_digest(&definition("results_are_honest", text)),
        statement_digest(&definition("plans_are_honest", text))
    );
    assert_ne!(
        statement_digest(&statement(StatementKind::Intent, text)),
        statement_digest(&statement(StatementKind::Channel, text))
    );
}

#[test]
fn channel_order_does_not_change_an_exception_digest() {
    let text = "The library returns the report value.";
    assert_eq!(
        statement_digest(&exception(
            "walk_exit_status",
            ExceptionKind::HoldsInstead,
            &["cli", "library"],
            text
        )),
        statement_digest(&exception(
            "walk_exit_status",
            ExceptionKind::HoldsInstead,
            &["library", "cli"],
            text
        ))
    );
}

const FIXED_TEST: &str = r#"/// Walk exits 1 when an "expectation" isn't met.
/// intent: example open_exits_one exemplifies walk_exit_status on cli
#[test]
fn open_exits_one() -> Result<(), String> {
    let codes: Vec<Vec<u8>> = vec![vec![1]]; // one code
    assert_eq!(codes[0][0], 1, "open");
    Ok(())
}
"#;

#[test]
fn the_token_serialization_of_a_fixed_test_equals_the_stored_string() {
    let item: syn::ItemFn = syn::parse_str(FIXED_TEST).unwrap();
    assert_eq!(
        canonical_example(&[], &item),
        concat!(
            "intent-digest/1\n",
            r#"# [ doc = " Walk exits 1 when an \"expectation\" isn't met." ] "#,
            r#"# [ doc = " intent: example open_exits_one exemplifies walk_exit_status on cli" ] "#,
            "# [ test ] fn open_exits_one ( ) -~ > Result < ( ) , String > { ",
            "let codes : Vec < Vec < u8 > > = vec ! [ vec ! [ 1 ] ] ; ",
            r#"assert_eq ! ( codes [ 0 ] [ 0 ] , 1 , "open" ) ; Ok ( ( ) ) }"#,
        )
    );
    assert_eq!(
        example_digest(&[], &item),
        "sha256:6130f956143f9bd53eba46651e9db21b98fb7366ae851ec7c190bd7963da7362"
    );
}

#[test]
fn the_attributes_around_a_test_lead_its_token_serialization() {
    let file: syn::File = syn::parse_str(&format!(
        "#![cfg(unix)]\n#[cfg(any())]\nmod checks {{\n{FIXED_TEST}}}\n"
    ))
    .unwrap();
    let syn::Item::Mod(module) = &file.items[0] else {
        panic!("expected a module");
    };
    let syn::Item::Fn(item) = &module.content.as_ref().unwrap().1[0] else {
        panic!("expected a function");
    };
    let context: Vec<&syn::Attribute> = file.attrs.iter().chain(&module.attrs).collect();
    let serialized = canonical_example(&context, item);
    assert_eq!(
        serialized.lines().take(2).collect::<Vec<_>>(),
        [
            "intent-digest/1",
            "context: # ! [ cfg ( unix ) ] # [ cfg ( any ( ) ) ]"
        ]
    );
    assert_eq!(
        serialized.lines().skip(2).collect::<Vec<_>>(),
        canonical_example(&[], item)
            .lines()
            .skip(1)
            .collect::<Vec<_>>()
    );
}
