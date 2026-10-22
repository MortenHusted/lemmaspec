//! Integration tests tagged as examples of the fixture catalog.

/// Walk exits 1 when an expectation is open.
/// intent: example open_exits_one exemplifies walk_exit_status on library, cli
#[test]
fn open_expectation_exits_one() {
    assert_eq!(1, 1);
}

#[test]
fn untagged_test_is_not_an_example() {}

/// intent: example helper_is_not_a_test exemplifies walk_exit_status on cli
fn helper() {}

mod exit_codes {
    mod invalid {
        /// intent: example invalid_exits_two exemplifies walk_exit_status on cli
        #[test]
        fn invalid_artifact_exits_two() {}
    }
}

/// intent: example library_report exemplifies library_returns_report on library
#[test]
fn library_returns_the_report() {}

/// intent: example undeclared exemplifies undeclared_statement on cli
#[test]
fn names_an_undeclared_statement() {}

/// intent: example honest exemplifies results_are_honest on cli
#[test]
fn names_an_intent() {}

/// intent: example no_web exemplifies no_web_exit_status on web
#[test]
fn names_a_does_not_apply_exception() {}

/// intent: example exemplifies walk_exit_status on cli
#[test]
fn has_a_malformed_tag() {}
