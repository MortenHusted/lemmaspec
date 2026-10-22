//! Integration tests tagged as examples of the approvals fixture catalog.

/// intent: example open_exits_one exemplifies walk_exit_status on library, cli
#[test]
fn open_expectation_exits_one() {
    assert_eq!(1, 1);
}

/// intent: example invalid_exits_two exemplifies walk_exit_status on cli
#[test]
fn invalid_artifact_exits_two() {
    assert_eq!(2, 2);
}
