use std::process::{Command, Output};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/catalog");

fn intent(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lemmaspec-intent"))
        .args(args)
        .current_dir(FIXTURES)
        .output()
        .unwrap()
}

#[test]
fn check_exits_zero_on_a_valid_catalog_and_names_the_error_otherwise() {
    for valid in [intent(&["check"]), intent(&["check", "--dir", "intent"])] {
        assert!(
            valid.status.success(),
            "{}",
            String::from_utf8_lossy(&valid.stderr)
        );
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
        assert!(!run.status.success(), "{args:?}");
        assert_eq!(
            String::from_utf8(run.stderr).unwrap(),
            format!("lemmaspec-intent: {error}\n")
        );
    }
}
