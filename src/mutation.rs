//! Deterministic mutation analysis for one self-contained artifact.

use serde::Serialize;

use crate::artifact::{
    evaluate_parsed_artifact, parse_artifact, Artifact, ArtifactError, MutationDecl,
    MutationOperator, WalkExpectation,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MutationTarget {
    Rule {
        rule: String,
    },
    Condition {
        rule: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        condition: Option<String>,
        index: usize,
        expression: String,
    },
    Fact {
        fact: String,
        relation: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationStatus {
    Killed,
    Survived,
    Rejected,
    Excluded,
}

impl MutationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Killed => "killed",
            Self::Survived => "survived",
            Self::Rejected => "rejected",
            Self::Excluded => "excluded",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MutationResult {
    pub id: String,
    pub policy: String,
    pub operator: MutationOperator,
    pub target: MutationTarget,
    pub status: MutationStatus,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub failed_expectations: Vec<WalkExpectation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MutationSummary {
    pub total: usize,
    pub executed: usize,
    pub killed: usize,
    pub survived: usize,
    pub rejected: usize,
    pub excluded: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MutationPolicyReport {
    pub id: String,
    pub operator: MutationOperator,
    pub status: String,
    pub summary: MutationSummary,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MutationReport {
    pub spec: String,
    pub status: String,
    pub baseline_status: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub baseline_failures: Vec<WalkExpectation>,
    pub summary: MutationSummary,
    pub policies: Vec<MutationPolicyReport>,
    pub mutations: Vec<MutationResult>,
}

pub fn mutate_artifact(source: &str) -> Result<MutationReport, ArtifactError> {
    let artifact = parse_artifact(source)?;
    if artifact.mutations.is_empty() {
        return Err(ArtifactError::new("artifact declares no mutation policies"));
    }

    let baseline = evaluate_parsed_artifact(artifact.clone())?.report;
    let baseline_failures: Vec<_> = baseline
        .expectations
        .iter()
        .filter(|expectation| !expectation.satisfied)
        .cloned()
        .collect();
    if !baseline_failures.is_empty() {
        return Ok(MutationReport {
            spec: artifact.name,
            status: "baseline_incomplete".to_string(),
            baseline_status: baseline.status,
            baseline_failures,
            summary: MutationSummary {
                total: 0,
                executed: 0,
                killed: 0,
                survived: 0,
                rejected: 0,
                excluded: 0,
            },
            policies: Vec::new(),
            mutations: Vec::new(),
        });
    }

    let mut results = Vec::new();
    for policy in &artifact.mutations {
        generate_policy_mutations(&artifact, policy, &mut results)?;
    }

    let summary = summarize(results.iter());
    let policies: Vec<_> = artifact
        .mutations
        .iter()
        .map(|policy| summarize_policy(policy, &results))
        .collect();
    let status = if summary.survived > 0 {
        "survived"
    } else if policies.iter().any(|policy| policy.status == "vacuous") {
        "vacuous"
    } else {
        "clean"
    };

    Ok(MutationReport {
        spec: artifact.name,
        status: status.to_string(),
        baseline_status: baseline.status,
        baseline_failures,
        summary,
        policies,
        mutations: results,
    })
}

fn generate_policy_mutations(
    artifact: &Artifact,
    policy: &MutationDecl,
    results: &mut Vec<MutationResult>,
) -> Result<(), ArtifactError> {
    match policy.operator {
        MutationOperator::DropRule => {
            for rule in &artifact.rules {
                let target = MutationTarget::Rule {
                    rule: rule.id.clone(),
                };
                let id = mutation_id(policy, &target);
                if policy.except.contains(&rule.id) {
                    results.push(excluded_result(id, policy, target));
                    continue;
                }
                let mut mutated = artifact.clone();
                mutated.rules.retain(|candidate| candidate.id != rule.id);
                results.push(evaluate_mutation(id, policy, target, mutated)?);
            }
        }
        MutationOperator::DropCondition => {
            for rule in &artifact.rules {
                for (index, expression) in rule.when.iter().enumerate() {
                    let condition = rule.condition_id(index).map(str::to_string);
                    let target = MutationTarget::Condition {
                        rule: rule.id.clone(),
                        condition: condition.clone(),
                        index: index + 1,
                        expression: expression.clone(),
                    };
                    let id = mutation_id(policy, &target);
                    if policy.excludes_condition(&rule.id, condition.as_deref()) {
                        results.push(excluded_result(id, policy, target));
                        continue;
                    }
                    let mut mutated = artifact.clone();
                    let mutated_rule = mutated
                        .rules
                        .iter_mut()
                        .find(|candidate| candidate.id == rule.id)
                        .expect("cloned artifact retains selected rule");
                    mutated_rule.remove_condition(index);
                    results.push(evaluate_mutation(id, policy, target, mutated)?);
                }
            }
        }
        MutationOperator::DropFact => {
            let relation = policy
                .relation
                .as_deref()
                .expect("validated drop_fact policy has a relation");
            for fact in artifact
                .facts
                .iter()
                .filter(|fact| fact.relation == relation)
            {
                let target = MutationTarget::Fact {
                    fact: fact.id.clone(),
                    relation: fact.relation.clone(),
                };
                let id = mutation_id(policy, &target);
                if policy.except.contains(&fact.id) {
                    results.push(excluded_result(id, policy, target));
                    continue;
                }
                let mut mutated = artifact.clone();
                mutated.facts.retain(|candidate| candidate.id != fact.id);
                results.push(evaluate_mutation(id, policy, target, mutated)?);
            }
        }
    }
    Ok(())
}

fn evaluate_mutation(
    id: String,
    policy: &MutationDecl,
    target: MutationTarget,
    artifact: Artifact,
) -> Result<MutationResult, ArtifactError> {
    match evaluate_parsed_artifact(artifact) {
        Ok(evaluation) => {
            let failed_expectations: Vec<_> = evaluation
                .report
                .expectations
                .into_iter()
                .filter(|expectation| !expectation.satisfied)
                .collect();
            let killed = match policy.must_fail.as_deref() {
                Some(required) => failed_expectations
                    .iter()
                    .any(|expectation| expectation.id == required),
                None => !failed_expectations.is_empty(),
            };
            Ok(MutationResult {
                id,
                policy: policy.id.clone(),
                operator: policy.operator,
                target,
                status: if killed {
                    MutationStatus::Killed
                } else {
                    MutationStatus::Survived
                },
                failed_expectations,
                diagnostic: None,
            })
        }
        Err(error) => mutation_error_result(id, policy, target, error),
    }
}

fn mutation_error_result(
    id: String,
    policy: &MutationDecl,
    target: MutationTarget,
    error: ArtifactError,
) -> Result<MutationResult, ArtifactError> {
    if !error.is_invalid_artifact() {
        return Err(error);
    }
    Ok(MutationResult {
        id,
        policy: policy.id.clone(),
        operator: policy.operator,
        target,
        status: MutationStatus::Rejected,
        failed_expectations: Vec::new(),
        diagnostic: Some(error.to_string()),
    })
}

fn excluded_result(id: String, policy: &MutationDecl, target: MutationTarget) -> MutationResult {
    MutationResult {
        id,
        policy: policy.id.clone(),
        operator: policy.operator,
        target,
        status: MutationStatus::Excluded,
        failed_expectations: Vec::new(),
        diagnostic: None,
    }
}

fn mutation_id(policy: &MutationDecl, target: &MutationTarget) -> String {
    match target {
        MutationTarget::Rule { rule } => {
            format!("{}:{}:{rule}", policy.id, policy.operator.as_str())
        }
        MutationTarget::Condition {
            rule,
            condition,
            index,
            ..
        } => {
            let condition = condition
                .as_deref()
                .map_or_else(|| index.to_string(), str::to_string);
            format!(
                "{}:{}:{rule}:{condition}",
                policy.id,
                policy.operator.as_str()
            )
        }
        MutationTarget::Fact { fact, .. } => {
            format!("{}:{}:{fact}", policy.id, policy.operator.as_str())
        }
    }
}

fn summarize<'a>(results: impl Iterator<Item = &'a MutationResult>) -> MutationSummary {
    let mut summary = MutationSummary {
        total: 0,
        executed: 0,
        killed: 0,
        survived: 0,
        rejected: 0,
        excluded: 0,
    };
    for result in results {
        summary.total += 1;
        match result.status {
            MutationStatus::Killed => {
                summary.executed += 1;
                summary.killed += 1;
            }
            MutationStatus::Survived => {
                summary.executed += 1;
                summary.survived += 1;
            }
            MutationStatus::Rejected => summary.rejected += 1,
            MutationStatus::Excluded => summary.excluded += 1,
        }
    }
    summary
}

fn summarize_policy(policy: &MutationDecl, results: &[MutationResult]) -> MutationPolicyReport {
    let summary = summarize(results.iter().filter(|result| result.policy == policy.id));
    let status = if summary.survived > 0 {
        "survived"
    } else if summary.executed == 0 {
        "vacuous"
    } else {
        "clean"
    };
    MutationPolicyReport {
        id: policy.id.clone(),
        operator: policy.operator,
        status: status.to_string(),
        summary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluator_errors_abort_instead_of_becoming_rejections() {
        let policy = MutationDecl {
            id: "coverage".to_string(),
            operator: MutationOperator::DropRule,
            relation: None,
            except: Vec::new(),
            must_fail: None,
            doc: None,
        };
        let target = MutationTarget::Rule {
            rule: "derive".to_string(),
        };

        let error = mutation_error_result(
            "coverage:drop_rule:derive".to_string(),
            &policy,
            target,
            ArtifactError::evaluation("engine failure"),
        )
        .expect_err("evaluation failure must abort mutation analysis");

        assert_eq!(error.to_string(), "engine failure");
    }
}
