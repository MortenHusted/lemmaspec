use lemmaspec::{bind_artifact, check_artifact, mutate_artifact, parse_artifact, print_artifact};
use lemmaspec_plan::{project_plan, CHECKER};

const PLAN: &str = include_str!("fixtures/plan.md");

#[test]
fn synthetic_projection_checks_and_binds_deterministically() {
    let evidence = project_plan(PLAN, "plan.md").unwrap();
    let text = print_artifact(&evidence);
    assert_eq!(
        text,
        print_artifact(&project_plan(PLAN, "plan.md").unwrap())
    );
    let report = check_artifact(CHECKER, &text).unwrap();
    assert_eq!(report.status, "clean", "{report:#?}");
    assert!(report
        .facts
        .iter()
        .any(|f| f.relation == "blocked" && f.args == ["u2", "u1"]));
    assert!(report
        .facts
        .iter()
        .any(|f| f.relation == "ready" && f.args == ["u1"]));
    assert!(!report.facts.iter().any(|f| f.relation == "accepted"));
    let bound = parse_artifact(&bind_artifact(CHECKER, &text).unwrap()).unwrap();
    assert!(bound.mutations.is_empty());
    assert!(evidence
        .facts
        .iter()
        .all(|f| f.provenance.len() == 1 && f.provenance[0].starts_with("plan.md#")));
    for (relation, anchor) in [
        ("deferred", "deferred"),
        ("semantic_input", "semantic-inputs"),
    ] {
        assert!(evidence
            .facts
            .iter()
            .any(|f| f.relation == relation && f.provenance == [format!("plan.md#{anchor}")]));
    }
}

#[test]
fn checker_self_test_and_mutation_policies_are_clean() {
    let report = mutate_artifact(CHECKER).unwrap();
    assert_eq!(report.status, "clean", "{report:#?}");
    assert!(report.summary.killed > 0);
}

#[test]
fn unknown_references_and_malformed_ranges_fail_closed() {
    for (from, to) in [
        ("RA1-RA2", "RA1-RA3"),
        ("RA1-RA2", "RA2-RA1"),
        ("RA1-RA2", "RA1-RB1"),
        ("Dependencies:** U1", "Dependencies:** U9"),
        ("Requirements:** RB1", "Requirements: RB1"),
    ] {
        let error = project_plan(&PLAN.replace(from, to), "plan.md").unwrap_err();
        assert!(!error.is_empty());
    }
}

#[test]
fn empty_coverage_is_a_failing_finding() {
    let text = print_artifact(&project_plan(&PLAN.replace("RA1-RA2", "None"), "plan.md").unwrap());
    let report = check_artifact(CHECKER, &text).unwrap();
    assert_ne!(report.status, "clean");
    assert!(report
        .facts
        .iter()
        .any(|f| f.relation == "uncovered_unit" && f.args == ["u1"]));
    assert!(report
        .facts
        .iter()
        .any(|f| f.relation == "uncovered" && f.args == ["ra1"]));
}

#[test]
fn renamed_heading_changes_provenance() {
    let evidence = project_plan(
        &PLAN.replace("U1. Parse input", "U1. Read input"),
        "plan.md",
    )
    .unwrap();
    assert!(evidence.facts.iter().any(|f| f.relation == "scheduled"
        && f.args == [lemmaspec::FactValue::Symbol("u1".into())]
        && f.provenance == ["plan.md#u1-read-input"]));
}

const STATUS: &str = r#"
spec status {
  fact target { relation: target_identity args: [current] }
  fact first_gate { relation: acceptance_claim args: [first_check, u1] }
  fact extra_gate { relation: acceptance_claim args: [extra_check, u1] }
  fact second_gate { relation: acceptance_claim args: [second_check, u2] }
  fact release_gate { relation: acceptance_claim args: [release_check, g1] }
  fact first_result { relation: observed args: [first_check, current] }
  fact extra_result { relation: observed args: [extra_check, current] }
  fact second_result { relation: observed args: [second_check, current] }
  fact release_result { relation: observed args: [release_check, current] }
}
"#;

fn with_status(status: &str) -> Result<String, String> {
    let artifact = lemmaspec_plan::compose_status(
        project_plan(PLAN, "plan.md")?,
        parse_artifact(status).map_err(|e| e.to_string())?,
    )?;
    Ok(print_artifact(&artifact))
}

#[test]
fn acceptance_requires_every_claim_at_exactly_one_target() {
    let complete = check_artifact(CHECKER, &with_status(STATUS).unwrap()).unwrap();
    assert_eq!(complete.status, "clean");
    assert!(complete
        .facts
        .iter()
        .any(|f| f.relation == "accepted" && f.args == ["plan"]));
    for status in [
        STATUS.replace(
            "fact extra_result { relation: observed args: [extra_check, current] }",
            "",
        ),
        STATUS.replace("args: [extra_check, current]", "args: [extra_check, old]"),
        STATUS
            .replace(
                "fact first_gate { relation: acceptance_claim args: [first_check, u1] }",
                "",
            )
            .replace(
                "fact extra_gate { relation: acceptance_claim args: [extra_check, u1] }",
                "",
            ),
    ] {
        let report = check_artifact(CHECKER, &with_status(&status).unwrap()).unwrap();
        assert_eq!(report.status, "incomplete");
        assert!(report
            .facts
            .iter()
            .any(|f| f.relation == "incomplete" && f.args == ["u1"]));
    }
    for status in [
        STATUS.replace(
            "fact target { relation: target_identity args: [current] }",
            "",
        ),
        STATUS.replace(
            "fact target",
            "fact another { relation: target_identity args: [other] } fact target",
        ),
        STATUS.replace("args: [first_check, u1]", "args: [first_check, u9]"),
        STATUS.replace("args: [extra_check, u1]", "args: [first_check, u1]"),
        STATUS.replace("relation: observed", "relation: scheduled"),
        STATUS.replace(
            "spec status {",
            "spec status { symbol u1 { label: \"Substitute label\" }",
        ),
        STATUS.replace(
            "spec status {",
            "spec status { relation injected { args: [symbol] }",
        ),
    ] {
        assert!(with_status(&status).is_err(), "{status}");
    }
}

#[test]
fn progress_without_claims_does_not_admit_a_unit() {
    let mut evidence = project_plan(PLAN, "plan.md").unwrap();
    evidence.facts.extend(
        parse_artifact(
            r#"spec progress {
      fact target { relation: target_identity args: [current] }
      fact file_exists { relation: observed args: [u1, current] }
    }"#,
        )
        .unwrap()
        .facts,
    );
    let report = check_artifact(CHECKER, &print_artifact(&evidence)).unwrap();
    assert!(report
        .facts
        .iter()
        .any(|f| f.relation == "incomplete" && f.args == ["u1"]));
    assert!(!report
        .facts
        .iter()
        .any(|f| f.relation == "accepted_evidence"));
}

#[test]
fn malformed_structure_never_disappears_silently() {
    for changed in [
        PLAN.replace("ce-unified-plan/v1", "ce-unified-plan/v2"),
        PLAN.replace(
            "## Gates\n- G1. Run the checks",
            "## Gates\nA gate described only in prose.",
        ),
        PLAN.replace(
            "## Deferred\n- D1. Additional formats",
            "## Deferred\n- None\n- D1. Additional formats",
        ),
        PLAN.replace("### U1. Parse input", "### U1 Parse input"),
        PLAN.replace("### U2. Publish result", "### U1. Publish result"),
        PLAN.replace("Dependencies:** None", "Dependencies:** U2"),
        PLAN.replace("Dependencies:** U1", "Dependencies:** U2"),
        PLAN.replace("RA1-RA2", "RA1, RA1"),
        PLAN.replace("RA1-RA2", "RA0-RA2"),
        PLAN.replace("RA1-RA2", "RA1-RA10001"),
        PLAN.replace("## Gates", "## Work"),
        PLAN.replace("## Gates", "## Checks"),
        PLAN.replace("## Gates", "```\n## Gates"),
        PLAN.replace(
            "**Requirements:** RB1.",
            "**Requirements:** RB1. **Requirements:** RB1.",
        ),
        PLAN.replace("- RA1. Parse the input", "- RA1 Parse the input"),
        PLAN.replace("- RA1. Parse the input", "**RA1. Parse the input**"),
    ] {
        assert!(
            project_plan(&changed, "plan.md").is_err(),
            "unexpectedly accepted:\n{changed}"
        );
    }
}

#[test]
fn markdown_snapshot_identity_tracks_bytes_not_paths() {
    let first = project_plan(PLAN, "one.md").unwrap();
    let renamed = project_plan(PLAN, "two.md").unwrap();
    let changed = project_plan(&format!("{PLAN}\n"), "one.md").unwrap();
    assert_eq!(
        first.facts[0].basis.as_ref().unwrap().identity,
        renamed.facts[0].basis.as_ref().unwrap().identity
    );
    assert_ne!(
        first.facts[0].basis.as_ref().unwrap().identity,
        changed.facts[0].basis.as_ref().unwrap().identity
    );
    assert!(first
        .facts
        .iter()
        .all(|f| f.basis.as_ref().unwrap().kind == lemmaspec::artifact::EvidenceKind::Snapshot));
}

#[test]
fn narrative_unit_ranges_are_not_structural_declarations() {
    let markdown = format!("{PLAN}\n## Completion\n- U1-U2 satisfy the acceptance criteria.\n- Rust2026 is a narrative label.\n");
    assert!(project_plan(&markdown, "plan.md").is_ok());
}

#[test]
fn fences_and_bold_list_sublabels_are_narrative_only() {
    for (open, close) in [("```mermaid", "```"), ("~~~~text", "~~~~~")] {
        let narrative = format!("{open}\n### U99. Pretend unit\n**Requirements:** R99. **Dependencies:** U98.\n## Requirements\n- R99. Pretend requirement\n```nested\n{close}\n");
        let input = PLAN
            .replace("## Work", &format!("{narrative}\n## Work"))
            .replace("- RA1.", "**Source and identity**\n- RA1.");
        let evidence = project_plan(&input, "plan.md").unwrap();
        assert_eq!(
            evidence.facts.len(),
            project_plan(PLAN, "plan.md").unwrap().facts.len()
        );
        assert!(!evidence
            .symbols
            .iter()
            .any(|symbol| symbol.value == "u99" || symbol.value == "r99"));
        assert_eq!(
            check_artifact(CHECKER, &print_artifact(&evidence))
                .unwrap()
                .status,
            "clean"
        );
    }
}

#[test]
fn external_gate_blocks_readiness_until_current_acceptance_is_observed() {
    let plan = PLAN.replace("Dependencies:** None", "Dependencies:** G1");
    for (status, blocked) in [
        (STATUS.to_string(), false),
        (
            STATUS.replace(
                "args: [release_check, current]",
                "args: [release_check, old]",
            ),
            true,
        ),
        (
            STATUS.replace(
                "fact release_result { relation: observed args: [release_check, current] }",
                "",
            ),
            true,
        ),
    ] {
        let evidence = lemmaspec_plan::compose_status(
            project_plan(&plan, "plan.md").unwrap(),
            parse_artifact(&status).unwrap(),
        )
        .unwrap();
        let report = check_artifact(CHECKER, &print_artifact(&evidence)).unwrap();
        assert_eq!(
            report
                .facts
                .iter()
                .any(|f| f.relation == "blocked" && f.args == ["u1", "g1"]),
            blocked
        );
        assert_eq!(
            report
                .facts
                .iter()
                .any(|f| f.relation == "ready" && f.args == ["u1"]),
            !blocked
        );
    }
    for invalid in ["G9", "G1-G2", "G1, G1", "D1"] {
        assert!(project_plan(
            &plan.replace("Dependencies:** G1", &format!("Dependencies:** {invalid}")),
            "plan.md"
        )
        .is_err());
    }
}
