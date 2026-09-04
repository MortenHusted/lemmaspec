//! Typed, deterministic specifications compiled to deductive logic.
//!
//! The initial engine is a focused port of Lemmalog's deterministic core.
//! Agent memory, LLM extraction, MCP, episodes, retrieval, and persistence
//! are deliberately outside this crate.

pub mod artifact;
pub mod ast;
pub mod eval;
pub mod html;
pub mod intern;
pub mod magic;
pub mod mutation;
pub mod narrative;
mod projection;

pub use artifact::{
    parse_artifact, walk_artifact, Artifact, ArtifactError, ExpectationDecl, FactDecl, FactValue,
    MutationDecl, MutationOperator, RelationDecl, RuleDecl, ValueType, WalkExpectation, WalkFact,
    WalkReport,
};
pub use ast::{parse_program, ParseError};
pub use eval::{Ann, Change, Engine, StoredFact, StratError};
pub use html::render_projection_html;
pub use intern::{Interner, Term, Value};
pub use mutation::{
    mutate_artifact, MutationPolicyReport, MutationReport, MutationResult, MutationStatus,
    MutationSummary, MutationTarget,
};
pub use narrative::read_fact;
pub use projection::{
    project_artifact, GraphEdge, GraphNode, GraphNodeData, GraphProjection, ProjectionError,
};

impl Engine {
    /// Install a versioned batch of rules and facts.
    pub fn install_program(&mut self, source: &str) -> Result<String, Box<dyn std::error::Error>> {
        let clauses = parse_program(source)?;
        self.validate_aggregates(&clauses)?;
        self.validate_rule_safety(&clauses)?;
        let previous_clause_count = self.clauses.len();
        let previous_ever_derived = self.ever_derived.clone();
        for (offset, clause) in clauses.iter().enumerate() {
            if !clause.is_fact {
                self.ever_derived.insert(clause.head.pred.clone());
                if clause
                    .head
                    .args
                    .iter()
                    .any(|term| matches!(term, Term::Agg(..)))
                {
                    let temp_clause = self.clauses.len() + offset;
                    self.ever_derived
                        .insert(format!("__agg:{}:{temp_clause}", clause.head.pred));
                }
            }
        }
        self.clauses.extend(clauses);
        if let Err(error) = self.check_program() {
            self.clauses.truncate(previous_clause_count);
            self.ever_derived = previous_ever_derived;
            return Err(error);
        }
        let id = format!("b{}", self.batch_counter);
        self.batch_counter += 1;
        self.rule_batches
            .push((id.clone(), source.to_string(), self.clauses.len()));
        self.program_dirty = true;
        Ok(id)
    }

    fn validate_aggregates(
        &self,
        clauses: &[crate::ast::Clause],
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::ast::Lit;

        fn term_has_aggregate(term: &Term) -> bool {
            matches!(term, Term::Agg(..))
        }

        for clause in clauses {
            if clause.is_fact {
                continue;
            }

            let has_aggregate = clause
                .head
                .args
                .iter()
                .any(|term| matches!(term, Term::Agg(..)));
            let mut seen_aggregate = false;
            for term in &clause.head.args {
                if matches!(term, Term::Agg(..)) {
                    seen_aggregate = true;
                } else if seen_aggregate {
                    return Err(format!(
                        "aggregate columns must be trailing in rule head {:?}",
                        clause.head.pred
                    )
                    .into());
                }
            }
            let mut bound = std::collections::BTreeSet::new();
            let mut aggregate_in_body = false;

            for literal in &clause.body {
                match literal {
                    Lit::Pos(atom) | Lit::Neg(atom) => {
                        for term in &atom.args {
                            if term_has_aggregate(term) {
                                aggregate_in_body = true;
                            }
                            if let Term::Var(variable) = term {
                                if matches!(literal, Lit::Pos(_)) {
                                    bound.insert(variable.as_str());
                                }
                            }
                        }
                    }
                    Lit::Cmp(_, term, expression) => {
                        if term_has_aggregate(term) {
                            aggregate_in_body = true;
                        }
                        check_expression(expression, &mut |term| {
                            if term_has_aggregate(term) {
                                aggregate_in_body = true;
                            }
                        });
                    }
                    Lit::Now(term) => {
                        if term_has_aggregate(term) {
                            aggregate_in_body = true;
                        }
                    }
                }
            }

            if aggregate_in_body {
                return Err(format!(
                    "aggregates are only allowed in rule heads: {:?}",
                    clause.head.pred
                )
                .into());
            }
            if !has_aggregate {
                continue;
            }

            let mut head_variables = Vec::new();
            for term in &clause.head.args {
                match term {
                    Term::Var(variable) => head_variables.push(variable.as_str()),
                    Term::Agg(_, inner) => {
                        if let Term::Var(variable) = &**inner {
                            head_variables.push(variable.as_str());
                        }
                    }
                    _ => {}
                }
            }
            let unbound: Vec<_> = head_variables
                .iter()
                .filter(|variable| !bound.contains(*variable))
                .copied()
                .collect();
            if !unbound.is_empty() {
                return Err(format!(
                    "unsafe aggregation rule {:?}: head variables {unbound:?} not bound by a positive body atom",
                    clause.head.pred
                )
                .into());
            }
        }
        Ok(())
    }

    fn validate_rule_safety(
        &self,
        clauses: &[crate::ast::Clause],
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::ast::Lit;
        use std::collections::BTreeSet;

        for clause in clauses {
            if clause.is_fact {
                continue;
            }

            let owner = clause.name.as_deref().unwrap_or(&clause.head.pred);
            let mut bound = BTreeSet::new();
            for literal in &clause.body {
                match literal {
                    Lit::Pos(atom) => {
                        for term in &atom.args {
                            collect_term_variables(term, &mut bound);
                        }
                    }
                    Lit::Neg(atom) => {
                        let mut used = BTreeSet::new();
                        for term in &atom.args {
                            collect_term_variables(term, &mut used);
                        }
                        reject_unbound(owner, "negation", &used, &bound)?;
                    }
                    Lit::Now(Term::Var(variable)) => {
                        bound.insert(variable.clone());
                    }
                    Lit::Now(_) => {}
                    Lit::Cmp(operator, left, expression) => {
                        let mut used = BTreeSet::new();
                        collect_term_variables(left, &mut used);
                        collect_expression_variables(expression, &mut used);

                        if let Term::Var(variable) = left {
                            if *operator == crate::ast::CmpOp::Eq
                                && !bound.contains(variable)
                                && expression_can_resolve(expression, &bound)
                            {
                                bound.insert(variable.clone());
                            }
                        }

                        if term_can_resolve(left, &bound) && *operator == crate::ast::CmpOp::Eq {
                            if let Some((1, Some(variable))) =
                                linear_expression_binding(expression, &bound)
                            {
                                bound.insert(variable);
                            }
                        }

                        reject_unbound(owner, "comparison", &used, &bound)?;
                    }
                }
            }

            let mut head_variables = BTreeSet::new();
            for term in &clause.head.args {
                collect_term_variables(term, &mut head_variables);
            }
            reject_unbound(owner, "head", &head_variables, &bound)?;
        }
        Ok(())
    }
}

fn collect_term_variables(term: &Term, variables: &mut std::collections::BTreeSet<String>) {
    match term {
        Term::Var(variable) => {
            variables.insert(variable.clone());
        }
        Term::Agg(_, inner) => collect_term_variables(inner, variables),
        _ => {}
    }
}

fn collect_expression_variables(
    expression: &crate::ast::Expr,
    variables: &mut std::collections::BTreeSet<String>,
) {
    use crate::ast::Expr;

    match expression {
        Expr::T(term) => collect_term_variables(term, variables),
        Expr::Add(left, right) | Expr::Sub(left, right) => {
            collect_expression_variables(left, variables);
            collect_expression_variables(right, variables);
        }
    }
}

fn reject_unbound(
    owner: &str,
    location: &str,
    used: &std::collections::BTreeSet<String>,
    bound: &std::collections::BTreeSet<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let unbound: Vec<&str> = used.difference(bound).map(String::as_str).collect();
    if unbound.is_empty() {
        Ok(())
    } else {
        Err(format!("unsafe rule {owner:?}: {location} variables {unbound:?} are unbound").into())
    }
}

fn term_can_resolve(term: &Term, bound: &std::collections::BTreeSet<String>) -> bool {
    match term {
        Term::Var(variable) => bound.contains(variable),
        Term::Int(_) | Term::Sym(_) => true,
        Term::Wildcard | Term::Agg(..) => false,
    }
}

fn expression_can_resolve(
    expression: &crate::ast::Expr,
    bound: &std::collections::BTreeSet<String>,
) -> bool {
    use crate::ast::Expr;

    match expression {
        Expr::T(term) => term_can_resolve(term, bound),
        Expr::Add(left, right) | Expr::Sub(left, right) => {
            numeric_expression_can_resolve(left, bound)
                && numeric_expression_can_resolve(right, bound)
        }
    }
}

fn numeric_expression_can_resolve(
    expression: &crate::ast::Expr,
    bound: &std::collections::BTreeSet<String>,
) -> bool {
    use crate::ast::Expr;

    match expression {
        Expr::T(Term::Int(_)) => true,
        Expr::T(Term::Var(variable)) => bound.contains(variable),
        Expr::T(_) => false,
        Expr::Add(left, right) | Expr::Sub(left, right) => {
            numeric_expression_can_resolve(left, bound)
                && numeric_expression_can_resolve(right, bound)
        }
    }
}

fn linear_expression_binding(
    expression: &crate::ast::Expr,
    bound: &std::collections::BTreeSet<String>,
) -> Option<(i64, Option<String>)> {
    use crate::ast::Expr;

    match expression {
        Expr::T(Term::Int(_)) => Some((0, None)),
        Expr::T(Term::Var(variable)) if bound.contains(variable) => Some((0, None)),
        Expr::T(Term::Var(variable)) => Some((1, Some(variable.clone()))),
        Expr::T(_) => None,
        Expr::Add(left, right) => {
            let (left_coefficient, left_variable) = linear_expression_binding(left, bound)?;
            let (right_coefficient, right_variable) = linear_expression_binding(right, bound)?;
            combine_linear_bindings(
                left_coefficient + right_coefficient,
                left_variable,
                right_variable,
            )
        }
        Expr::Sub(left, right) => {
            let (left_coefficient, left_variable) = linear_expression_binding(left, bound)?;
            let (right_coefficient, right_variable) = linear_expression_binding(right, bound)?;
            combine_linear_bindings(
                left_coefficient - right_coefficient,
                left_variable,
                right_variable,
            )
        }
    }
}

fn combine_linear_bindings(
    coefficient: i64,
    left: Option<String>,
    right: Option<String>,
) -> Option<(i64, Option<String>)> {
    let variable = match (left, right) {
        (None, None) => None,
        (Some(variable), None) | (None, Some(variable)) => Some(variable),
        (Some(_), Some(_)) => return None,
    };
    Some((coefficient, variable))
}

fn check_expression<F: FnMut(&Term)>(expression: &crate::ast::Expr, visit: &mut F) {
    use crate::ast::Expr;

    match expression {
        Expr::T(term) => visit(term),
        Expr::Add(left, right) | Expr::Sub(left, right) => {
            check_expression(left, visit);
            check_expression(right, visit);
        }
    }
}
