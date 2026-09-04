//! Apply a checker's relations and rules to evidence supplied by another file.
//!
//! A checker is an ordinary self-contained artifact: its own facts are a
//! fixture and its expectations and mutation policies are its self-test. A
//! check replaces that fixture with the evidence file's facts and evaluates
//! the evidence file's expectations instead. The result is one closed
//! artifact evaluated exactly like a walk; nothing is resolved by reference.

use crate::artifact::{
    evaluate_parsed_artifact, parse_artifact, Artifact, ArtifactError, WalkReport,
};

/// Evaluate `checker`'s vocabulary and rules over `evidence`'s facts and
/// expectations. The evidence file may declare only facts and expectations.
pub fn check_artifact(checker: &str, evidence: &str) -> Result<WalkReport, ArtifactError> {
    let checker = parse_artifact(checker)?;
    let evidence = parse_artifact(evidence)?;
    Ok(evaluate_parsed_artifact(bind(checker, evidence)?)?.report)
}

fn bind(checker: Artifact, evidence: Artifact) -> Result<Artifact, ArtifactError> {
    // A mutation cannot be parsed without a rule or relation to target, so
    // rejecting those two is enough to keep the evidence to facts and
    // expectations.
    for (kind, declared) in [
        ("relation", !evidence.relations.is_empty()),
        ("rule", !evidence.rules.is_empty()),
    ] {
        if declared {
            return Err(ArtifactError::new(format!(
                "evidence `{}` declares a {kind}; evidence may only assert facts and expectations",
                evidence.name
            )));
        }
    }

    Ok(Artifact {
        name: evidence.name,
        doc: evidence.doc,
        relations: checker.relations,
        facts: evidence.facts,
        rules: checker.rules,
        expectations: evidence.expectations,
        mutations: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHECKER: &str = r#"
        spec checker {
          relation input { args: [symbol] }
          relation flagged { args: [symbol] }
          fact fixture { relation: input args: [fixture] }
          rule flag_every_input { derive: "flagged(X)" when: ["input(X)"] }
          expect fixture_is_flagged { query: "flagged(fixture)" count: 1 }
          mutation rules_bite { operator: drop_rule }
        }
    "#;

    #[test]
    fn evidence_replaces_the_fixture_and_its_expectations() {
        let evidence = r#"
            spec evidence {
              fact real { relation: input args: [real] }
              expect real_is_flagged { query: "flagged(real)" count: 1 }
              expect fixture_is_gone { query: "flagged(fixture)" count: 0 }
            }
        "#;

        let report = check_artifact(CHECKER, evidence).expect("check succeeds");

        assert_eq!(report.spec, "evidence");
        assert_eq!(report.status, "clean");
        assert_eq!(report.asserted, 1);
        assert_eq!(report.derived, 1);
        assert_eq!(report.expectations.len(), 2);
    }

    #[test]
    fn evidence_is_validated_against_the_checker_vocabulary() {
        let evidence = r#"
            spec evidence {
              fact stray { relation: unknown args: [x] }
            }
        "#;

        let error = check_artifact(CHECKER, evidence).expect_err("unknown relation is rejected");

        assert!(
            error.to_string().contains("unknown relation `unknown`"),
            "{error}"
        );
    }

    #[test]
    fn evidence_may_not_redefine_the_checker() {
        for (declaration, kind) in [
            ("relation extra { args: [symbol] }", "relation"),
            (
                r#"rule extra { derive: "flagged(X)" when: ["input(X)"] }"#,
                "rule",
            ),
        ] {
            let evidence = format!("spec evidence {{ {declaration} }}");

            let error = check_artifact(CHECKER, &evidence).expect_err("declaration is rejected");

            assert!(
                error.to_string().contains(&format!("declares a {kind}")),
                "{kind}: {error}"
            );
        }
    }
}
