//! Optional intent catalog for LemmaSpec. LemmaSpec's core does not depend on it.

use std::collections::BTreeMap;

pub mod approvals;
pub mod digest;
pub mod examples;
pub mod identity;
pub mod loader;
pub mod project;
pub mod recorder;
pub mod report;
pub mod runs;
pub mod syntax;

/// The bundled checker that decides standings over projected evidence.
pub const CHECKER: &str = include_str!("../intent_checker.lemmaspec");

/// A validated catalog: every statement reachable from the root, keyed by its
/// catalog-wide unique ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    pub statements: BTreeMap<String, Statement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    pub id: String,
    pub text: String,
    /// The declaring file, relative to the catalog directory with `/` separators.
    pub file: String,
    pub kind: StatementKind,
}

/// Links are resolved to the IDs of the statements they depend on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatementKind {
    Intent,
    Definition {
        refines: String,
    },
    Exception {
        narrows: String,
        kind: ExceptionKind,
        channels: Vec<String>,
    },
    Channel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExceptionKind {
    DoesNotApply,
    HoldsInstead,
}

impl ExceptionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ExceptionKind::DoesNotApply => "does_not_apply",
            ExceptionKind::HoldsInstead => "holds_instead",
        }
    }
}
