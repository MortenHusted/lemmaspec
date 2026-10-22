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

    pub fn parse(text: &str) -> Option<ExceptionKind> {
        match text {
            "does_not_apply" => Some(ExceptionKind::DoesNotApply),
            "holds_instead" => Some(ExceptionKind::HoldsInstead),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ItemKind {
    Intent,
    Definition,
    Exception,
    Channel,
    Example,
}

impl ItemKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ItemKind::Intent => "intent",
            ItemKind::Definition => "definition",
            ItemKind::Exception => "exception",
            ItemKind::Channel => "channel",
            ItemKind::Example => "example",
        }
    }

    pub fn parse(text: &str) -> Option<ItemKind> {
        match text {
            "intent" => Some(ItemKind::Intent),
            "definition" => Some(ItemKind::Definition),
            "exception" => Some(ItemKind::Exception),
            "channel" => Some(ItemKind::Channel),
            "example" => Some(ItemKind::Example),
            _ => None,
        }
    }

    pub(crate) fn of(kind: &StatementKind) -> ItemKind {
        match kind {
            StatementKind::Intent => ItemKind::Intent,
            StatementKind::Definition { .. } => ItemKind::Definition,
            StatementKind::Exception { .. } => ItemKind::Exception,
            StatementKind::Channel => ItemKind::Channel,
        }
    }
}
