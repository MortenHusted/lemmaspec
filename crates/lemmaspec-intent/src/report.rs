//! The verified-statement report: projected evidence bound to the bundled
//! checker, read back as standings, reasons and changes awaiting approval,
//! and rendered for a reader. Only files and git objects are read; no
//! project code runs (KTD11).

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use lemmaspec::{
    bind_parsed_artifact, parse_artifact, print_artifact, project_parsed_artifact,
    render_projection_html_with_context, walk_parsed_artifact, Artifact, SourceContext, WalkReport,
};

use crate::{
    approvals::{Action, Log},
    digest::DIGEST_VERSION,
    identity::Repo,
    loader::{APPROVALS_FILE, RUNS_FILE},
    project::{project, read_or_empty, Inputs},
    runs::{self, Outcome},
    StatementKind, CHECKER,
};

pub struct Options {
    /// The catalog directory.
    pub dir: PathBuf,
    /// The directory of tagged integration tests.
    pub tests: PathBuf,
    /// Run ledgers read in addition to the catalog ledger.
    pub ledgers: Vec<PathBuf>,
    /// The commit whose approvals and failing runs the report compares with.
    pub base: Option<String>,
}

pub struct Report {
    /// The evidence bound to the checker: the proof record behind every
    /// standing, which `lemmaspec walk` and `render` accept unchanged.
    pub bound: Artifact,
    pub markdown: String,
    /// Whether an expectation of the bound evidence is open, as one is while
    /// anything awaits approval.
    pub pending: bool,
    /// Each intent, definition and exception with its standing.
    pub standings: BTreeMap<String, String>,
    /// The evaluated source identity: the working tree's.
    pub target: String,
}

pub fn report(options: &Options) -> Result<Report, String> {
    let inputs = Inputs::read(&options.dir, &options.tests, &options.ledgers)?;
    let repo = Repo::of_catalog(&options.dir)?;
    let target = repo.working_tree_identity()?;
    let head = repo.head()?;
    let since = match &options.base {
        Some(base) => Some(since(&repo, base, &options.dir, &inputs)?),
        None => None,
    };
    let header = if repo.commit_identity(&head)? == target {
        format!("Source identity `{target}` matches the tree of HEAD commit `{head}`.")
    } else {
        format!("Source identity `{target}` is the working tree, which does not match the tree of HEAD commit `{head}`, so these standings do not hold for that commit.")
    };
    let checker = parse_artifact(CHECKER).map_err(|e| e.to_string())?;
    let bound =
        bind_parsed_artifact(&checker, &project(&inputs, &target)).map_err(|e| e.to_string())?;
    let walk = walk_parsed_artifact(&bound).map_err(|e| e.to_string())?;
    let found = Found::read(&walk, checker);

    let mut sections = vec![
        format!("# Intent report\n\n{header}\n"),
        awaiting(&found),
        statements(&inputs, &found),
    ];
    sections.extend(ignored(&inputs));
    sections.extend(not_evaluated(&inputs));
    sections.extend(since);
    Ok(Report {
        markdown: sections.join("\n"),
        pending: walk.status != "clean",
        standings: found.standings,
        bound,
        target,
    })
}

impl Report {
    /// The bound evidence rendered by LemmaSpec's HTML renderer for the
    /// evaluated source identity.
    pub fn html(&self, context: Option<&SourceContext>) -> Result<String, String> {
        let projection = project_parsed_artifact(&self.bound).map_err(|e| e.to_string())?;
        Ok(render_projection_html_with_context(
            &print_artifact(&self.bound),
            &projection,
            Some(&self.target),
            context,
        ))
    }
}

/// What the walk of the bound evidence derived, with the checker's labels
/// for its reasons.
#[derive(Default)]
struct Found {
    standings: BTreeMap<String, String>,
    kinds: BTreeMap<String, String>,
    /// Each judged item's `(reason, subject)` pairs.
    reasons: BTreeMap<String, Vec<(String, String)>>,
    /// Each item awaiting approval with its `(reason, subject)` pairs.
    awaiting: BTreeMap<String, Vec<(String, String)>>,
    labels: BTreeMap<String, String>,
}

impl Found {
    fn read(walk: &WalkReport, checker: Artifact) -> Found {
        let mut found = Found {
            labels: checker
                .symbols
                .into_iter()
                .map(|symbol| (symbol.value, symbol.label))
                .collect(),
            ..Found::default()
        };
        let mut awaiting = BTreeSet::new();
        let mut why: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
        for fact in &walk.facts {
            match (fact.relation.as_str(), fact.args.as_slice()) {
                ("standing", [item, standing]) => {
                    found.standings.insert(item.clone(), standing.clone());
                }
                ("present", [item, kind]) => {
                    found.kinds.insert(item.clone(), kind.clone());
                }
                ("reason", [item, reason, subject]) => found
                    .reasons
                    .entry(item.clone())
                    .or_default()
                    .push((reason.clone(), subject.clone())),
                ("awaiting_approval", [item]) => {
                    awaiting.insert(item.clone());
                }
                ("approval_reason", [item, reason, subject]) => why
                    .entry(item.clone())
                    .or_default()
                    .push((reason.clone(), subject.clone())),
                ("stale_link", [item, target]) => why
                    .entry(item.clone())
                    .or_default()
                    .push(("linked_statement_changed".into(), target.clone())),
                _ => {}
            }
        }
        found.awaiting = awaiting
            .into_iter()
            .map(|item| {
                let reasons = why.remove(&item).unwrap_or_default();
                (item, reasons)
            })
            .collect();
        found
    }

    fn kind(&self, item: &str) -> &str {
        self.kinds.get(item).map_or("item", String::as_str)
    }

    /// A reason in the checker's words, naming its subject unless that is
    /// the item itself.
    fn describe(&self, item: &str, reason: &str, subject: &str) -> String {
        let label = self.labels.get(reason).map_or(reason, String::as_str);
        if subject == item {
            label.to_string()
        } else {
            format!("{label} (`{subject}`)")
        }
    }
}

/// Every change awaiting approval (R16).
fn awaiting(found: &Found) -> String {
    let mut out = String::from("## Awaiting approval\n\n");
    if found.awaiting.is_empty() {
        out.push_str("Nothing awaits approval.\n");
    }
    for (item, reasons) in &found.awaiting {
        let reasons: Vec<String> = reasons
            .iter()
            .map(|(reason, subject)| found.describe(item, reason, subject))
            .collect();
        out.push_str(&format!(
            "- `{item}` ({}): {}\n",
            found.kind(item),
            reasons.join("; ")
        ));
    }
    out
}

/// Every intent, definition and exception with its prose, links, standing
/// and reasons (R17, R22, R24), in ID order.
fn statements(inputs: &Inputs, found: &Found) -> String {
    let mut blocks = vec![String::from("## Statements\n")];
    if found.standings.is_empty() {
        blocks.push("No intents, definitions or exceptions.\n".into());
    }
    for (id, standing) in &found.standings {
        let kind = found.kind(id);
        let mut block = format!("### `{id}` ({kind}): {standing}\n\n");
        match inputs.catalog.statements.get(id) {
            Some(statement) => {
                block.push_str(&quote(&statement.text));
                block.push_str(&format!(
                    "- File: `{}`\n",
                    inputs.catalog_file(&statement.file)
                ));
                match &statement.kind {
                    StatementKind::Definition { refines } => {
                        block.push_str(&format!("- Refines: `{refines}`\n"));
                    }
                    StatementKind::Exception {
                        narrows,
                        kind,
                        channels,
                    } => block.push_str(&format!(
                        "- Narrows: `{narrows}`\n- Kind: {} on {}\n",
                        kind.as_str(),
                        ticked(channels)
                    )),
                    StatementKind::Intent | StatementKind::Channel => {}
                }
            }
            None => {
                let entry = inputs
                    .evaluated_entries()
                    .map(|(_, entry)| entry)
                    .filter(|entry| entry.item == *id)
                    .last();
                if let Some(text) = entry.and_then(|entry| entry.text.as_ref()) {
                    block.push_str(&quote(text));
                }
                block.push_str("- File: removed from the catalog; its approval stands until a removal is approved\n");
                let link = if kind == "definition" {
                    "Refines"
                } else {
                    "Narrows"
                };
                for target in entry.into_iter().flat_map(|entry| entry.links.keys()) {
                    block.push_str(&format!("- {link}: `{target}`\n"));
                }
            }
        }
        let examples: Vec<String> = inputs
            .examples
            .examples
            .iter()
            .filter(|example| example.statement == *id)
            .map(|example| format!("`{}` on {}", example.id, ticked(&example.channels)))
            .collect();
        if !examples.is_empty() {
            block.push_str(&format!("- Examples: {}\n", examples.join("; ")));
        }
        for (reason, subject) in found.reasons.get(id).into_iter().flatten() {
            block.push_str(&format!(
                "- Reason: {}\n",
                found.describe(id, reason, subject)
            ));
        }
        blocks.push(block);
    }
    blocks.join("\n")
}

/// Tagged tests that do not count as examples, with why (R28).
fn ignored(inputs: &Inputs) -> Option<String> {
    if inputs.examples.issues.is_empty() {
        return None;
    }
    let mut out = String::from("## Ignored examples\n\n");
    for issue in &inputs.examples.issues {
        let at = format!("`{}:{}`", inputs.test_file(&issue.file), issue.line);
        let example = match &issue.example {
            Some(id) => format!("`{id}` at {at}"),
            None => at,
        };
        out.push_str(&format!(
            "- {example} (`{}`): {}\n",
            issue.test, issue.reason
        ));
    }
    Some(out)
}

/// Approvals and runs recorded under another digest version, which the
/// evidence leaves out (KTD4).
fn not_evaluated(inputs: &Inputs) -> Option<String> {
    let mut lines = Vec::new();
    for (index, entry) in inputs.log.entries.iter().enumerate() {
        if entry.digest_version != DIGEST_VERSION {
            lines.push(format!(
                "- Approval entry {} for `{}` by {}: recorded under digest version {}\n",
                index + 1,
                entry.item,
                entry.by,
                entry.digest_version
            ));
        }
    }
    for ledger in &inputs.ledgers {
        for run in &ledger.runs {
            if run.digest_version != DIGEST_VERSION {
                lines.push(format!(
                    "- Run of `{}` on `{}` in `{}`: recorded under digest version {}\n",
                    run.example, run.channel, ledger.path, run.digest_version
                ));
            }
        }
    }
    (!lines.is_empty()).then(|| format!("## Not evaluated\n\n{}", lines.concat()))
}

/// The approvals and failing runs recorded in the catalog since `base`
/// (R27): whatever the catalog files hold beyond their content at `base`, or
/// a note that a file was altered when that content is not a byte prefix of
/// the current one.
fn since(repo: &Repo, base: &str, dir: &Path, inputs: &Inputs) -> Result<String, String> {
    let at_base = |name: &str| -> Result<String, String> {
        Ok(repo.catalog_file_at(base, name)?.unwrap_or_default())
    };
    let altered = |file: &str| {
        format!("The {file} was altered: its content at the base is not a prefix of its content now, so what was added since cannot be told apart. Review the whole {file}.\n")
    };
    let mut out = format!("## Since base `{base}`\n\n### Approvals\n\n");
    let then = at_base(APPROVALS_FILE)?;
    if read_or_empty(&dir.join(APPROVALS_FILE))?.starts_with(&then) {
        let before = Log::parse(&then)
            .map_err(|e| format!("{base}:{e}"))?
            .entries
            .len();
        let added: Vec<String> = inputs
            .log
            .entries
            .iter()
            .enumerate()
            .skip(before)
            .map(|(index, entry)| {
                let verb = match entry.action {
                    Action::Approve => "approved",
                    Action::Remove => "removed",
                };
                format!(
                    "- Entry {}: {} {verb} {} `{}`\n",
                    index + 1,
                    entry.by,
                    entry.kind.as_str(),
                    entry.item
                )
            })
            .collect();
        out.push_str(&none_or(added.concat()));
    } else {
        out.push_str(&altered("approval log"));
    }
    out.push_str("\n### Failing runs\n\n");
    let then = at_base(RUNS_FILE)?;
    if read_or_empty(&dir.join(RUNS_FILE))?.starts_with(&then) {
        let before = runs::parse(&then, &format!("{base}:{RUNS_FILE}"))?.len();
        let failures: Vec<String> = inputs.ledgers[0]
            .runs
            .iter()
            .skip(before)
            .filter(|run| run.outcome == Outcome::Fail)
            .map(|run| {
                let diff = run.diff.as_deref().unwrap_or_default();
                let fence = fence(diff);
                let newline = if diff.is_empty() || diff.ends_with('\n') {
                    ""
                } else {
                    "\n"
                };
                format!(
                    "`{}` failed on `{}` at source `{}`, based on `{}`:\n\n{fence}diff\n{diff}{newline}{fence}\n",
                    run.example, run.channel, run.source, run.base
                )
            })
            .collect();
        out.push_str(&none_or(failures.join("\n")));
    } else {
        out.push_str(&altered("run ledger"));
    }
    Ok(out)
}

fn none_or(text: String) -> String {
    if text.is_empty() {
        "None.\n".into()
    } else {
        text
    }
}

/// A code fence longer than any run of backticks in `text`.
fn fence(text: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    "`".repeat(longest.max(2) + 1)
}

fn quote(text: &str) -> String {
    format!("> {}\n\n", text.replace('\n', "\n> "))
}

fn ticked(ids: &[String]) -> String {
    ids.iter()
        .map(|id| format!("`{id}`"))
        .collect::<Vec<_>>()
        .join(", ")
}
