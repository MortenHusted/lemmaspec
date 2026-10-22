use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use lemmaspec::{
    Artifact, EvidenceBasis, EvidenceKind, ExpectationDecl, FactDecl, FactValue, SymbolDecl,
};

use crate::{
    approvals::{Entry, Log},
    digest::{sha256, statement_digest, DIGEST_VERSION},
    examples::{scan, Scan},
    loader::{load, missing_as_empty, DirReader, APPROVALS_FILE, RUNS_FILE},
    runs::{self, Run},
    Catalog, ItemKind, StatementKind,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ledger {
    pub path: String,
    pub runs: Vec<Run>,
}

impl Ledger {
    pub fn evaluated_runs(&self) -> impl Iterator<Item = &Run> {
        self.runs
            .iter()
            .filter(|run| run.digest_version == DIGEST_VERSION)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inputs {
    pub catalog: Catalog,
    pub examples: Scan,
    pub log: Log,
    pub catalog_ledger: Ledger,
    pub extra_ledgers: Vec<Ledger>,
    pub(crate) dir: PathBuf,
    tests: PathBuf,
    digest_by_cited_path: BTreeMap<String, String>,
}

impl Inputs {
    /// Read the catalog in `dir` with its approval log and run ledger, the
    /// examples in `tests`, and each extra ledger, where a missing one holds
    /// no runs.
    pub fn read(dir: &Path, tests: &Path, ledgers: &[PathBuf]) -> Result<Inputs, String> {
        let reader = DirReader(dir.into());
        let catalog = load(&reader)?;
        let examples = scan(tests, &catalog)?;
        let log = Log::read(&reader)?;
        let catalog_ledger = Ledger {
            path: cited(&dir.join(RUNS_FILE)),
            runs: runs::read(&reader)?,
        };
        let mut extra_ledgers = Vec::new();
        for path in ledgers {
            extra_ledgers.push(Ledger {
                path: cited(path),
                runs: runs::parse(&read_or_empty(path)?, &cited(path))?,
            });
        }
        let mut inputs = Inputs {
            catalog,
            examples,
            log,
            catalog_ledger,
            extra_ledgers,
            dir: dir.into(),
            tests: tests.into(),
            digest_by_cited_path: BTreeMap::new(),
        };
        let mut files: Vec<PathBuf> = inputs
            .catalog
            .statements
            .values()
            .map(|statement| dir.join(&statement.file))
            .chain(
                inputs
                    .examples
                    .examples
                    .iter()
                    .map(|example| test_path(tests, &example.file)),
            )
            .chain([dir.join(APPROVALS_FILE), dir.join(RUNS_FILE)])
            .chain(ledgers.iter().cloned())
            .collect();
        files.sort();
        files.dedup();
        for file in files {
            match fs::read_to_string(&file) {
                Ok(text) => {
                    inputs
                        .digest_by_cited_path
                        .insert(cited(&file), sha256(&text));
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(format!("{}: {e}", file.display())),
            }
        }
        Ok(inputs)
    }

    /// The path facts cite for a catalog file, given relative to the catalog.
    pub fn catalog_file(&self, file: &str) -> String {
        cited(&self.dir.join(file))
    }

    /// The path facts cite for the test file with this stem.
    pub fn test_file(&self, stem: &str) -> String {
        cited(&test_path(&self.tests, stem))
    }

    /// Log entries with their positions, counted from 1 in file order, that
    /// were recorded under the current digest version.
    pub fn evaluated_entries(&self) -> impl Iterator<Item = (usize, &Entry)> {
        self.log
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| (index + 1, entry))
            .filter(|(_, entry)| entry.digest_version == DIGEST_VERSION)
    }

    /// The catalog ledger, then each extra ledger in the order given.
    pub fn ledgers(&self) -> impl Iterator<Item = &Ledger> {
        std::iter::once(&self.catalog_ledger).chain(&self.extra_ledgers)
    }

    fn snapshot(&self, path: String) -> EvidenceBasis {
        let digest = self.digest_by_cited_path[&path].clone();
        snapshot(path, digest)
    }
}

pub(crate) fn read_or_empty(path: &Path) -> Result<String, String> {
    missing_as_empty(fs::read_to_string(path), path.display())
}

fn test_path(tests: &Path, stem: &str) -> PathBuf {
    tests.join(format!("{stem}.rs"))
}

fn cited(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn snapshot(source: String, identity: String) -> EvidenceBasis {
    EvidenceBasis {
        kind: EvidenceKind::Snapshot,
        source,
        identity: Some(identity),
    }
}

const WORKING_TREE: &str = "working tree";

/// The evidence for `inputs` evaluated at the source identity `target`, with
/// the invariant expectations: nothing awaits approval, and every intent,
/// definition and exception that counts has exactly one standing.
pub fn project(inputs: &Inputs, target: &str) -> Artifact {
    let mut facts = Facts::default();
    let mut symbols = Vec::new();
    let mut judged = Vec::new();
    for statement in inputs.catalog.statements.values() {
        let basis = inputs.snapshot(inputs.catalog_file(&statement.file));
        let id = statement.id.as_str();
        symbols.push(SymbolDecl {
            value: id.into(),
            label: statement.text.clone(),
            source: Some(basis.source.clone()),
            doc: None,
        });
        let kind = ItemKind::of(&statement.kind);
        facts.add(&basis, "item", &[id, kind.as_str()]);
        facts.add(
            &basis,
            "current_digest",
            &[id, &statement_digest(statement)],
        );
        match &statement.kind {
            StatementKind::Definition { refines } => facts.add(&basis, "refines", &[id, refines]),
            StatementKind::Exception {
                narrows,
                kind,
                channels,
            } => {
                facts.add(&basis, "narrows", &[id, narrows]);
                facts.add(&basis, "exception_kind", &[id, kind.as_str()]);
                for channel in channels {
                    facts.add(&basis, "scoped_to", &[id, channel]);
                }
            }
            StatementKind::Intent | StatementKind::Channel => {}
        }
        if kind != ItemKind::Channel {
            judged.push(id.to_string());
        }
    }
    for example in &inputs.examples.examples {
        let basis = inputs.snapshot(inputs.test_file(&example.file));
        let id = example.id.as_str();
        symbols.push(SymbolDecl {
            value: id.into(),
            label: format!("Test {} in {}.rs", example.test, example.file),
            source: Some(basis.source.clone()),
            doc: None,
        });
        facts.add(&basis, "item", &[id, ItemKind::Example.as_str()]);
        facts.add(&basis, "current_digest", &[id, &example.digest]);
        facts.add(&basis, "exemplifies", &[id, &example.statement]);
        for channel in &example.channels {
            facts.add(&basis, "tagged", &[id, channel]);
        }
    }
    let evaluated: Vec<(usize, &Entry)> = inputs.evaluated_entries().collect();
    if !evaluated.is_empty() {
        let basis = inputs.snapshot(inputs.catalog_file(APPROVALS_FILE));
        for &(position, entry) in &evaluated {
            let position = FactValue::Integer(position as i64);
            facts.push(
                &basis,
                "log_entry",
                vec![
                    position.clone(),
                    symbol(&entry.item),
                    symbol(entry.kind.as_str()),
                    symbol(entry.action.as_str()),
                    symbol(&entry.digest),
                ],
            );
            for (target, digest) in &entry.links {
                facts.push(
                    &basis,
                    "log_link",
                    vec![position.clone(), symbol(target), symbol(digest)],
                );
            }
            for channel in &entry.channels {
                facts.push(&basis, "log_tag", vec![position.clone(), symbol(channel)]);
            }
        }
    }
    let evaluated = Log {
        entries: evaluated
            .into_iter()
            .map(|(_, entry)| entry.clone())
            .collect(),
    };
    for entry in evaluated.reinstated(&inputs.catalog, &inputs.examples) {
        if matches!(entry.kind, ItemKind::Channel | ItemKind::Example) {
            continue;
        }
        if let Some(text) = &entry.text {
            symbols.push(SymbolDecl {
                value: entry.item.clone(),
                label: text.clone(),
                source: None,
                doc: None,
            });
        }
        judged.push(entry.item.clone());
    }
    for ledger in inputs.ledgers() {
        let runs: Vec<&Run> = ledger.evaluated_runs().collect();
        if runs.is_empty() {
            continue;
        }
        let basis = inputs.snapshot(ledger.path.clone());
        for run in runs {
            facts.add(
                &basis,
                "run",
                &[
                    &run.example,
                    &run.digest,
                    &run.channel,
                    &run.source,
                    run.outcome.as_str(),
                ],
            );
        }
    }
    let basis = snapshot(WORKING_TREE.into(), target.into());
    facts.add(&basis, "target_identity", &[target]);

    let mut expectations = vec![ExpectationDecl {
        id: "nothing_awaits_approval".into(),
        query: "awaiting_approval(I)".into(),
        count: 0,
        doc: None,
    }];
    judged.sort();
    for id in judged {
        expectations.push(ExpectationDecl {
            id: format!("one_standing_{id}"),
            query: format!("standing(\"{id}\", Kind)"),
            count: 1,
            doc: None,
        });
    }
    symbols.sort_by(|a, b| a.value.cmp(&b.value));
    Artifact {
        name: "intent_evidence".into(),
        doc: Some("Which intents, domain definitions and exceptions are verified at the target source identity, and which changes await approval?".into()),
        notes: None,
        symbols,
        relations: Vec::new(),
        facts: facts.declarations(),
        rules: Vec::new(),
        expectations,
        mutations: Vec::new(),
    }
}

fn symbol(value: &str) -> FactValue {
    FactValue::Symbol(value.into())
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Fact {
    relation: String,
    args: Vec<FactValue>,
    basis: EvidenceBasis,
}

#[derive(Default)]
struct Facts(Vec<Fact>);

impl Facts {
    fn add(&mut self, basis: &EvidenceBasis, relation: &str, args: &[&str]) {
        let args = args.iter().map(|arg| symbol(arg)).collect();
        self.push(basis, relation, args);
    }

    fn push(&mut self, basis: &EvidenceBasis, relation: &str, args: Vec<FactValue>) {
        self.0.push(Fact {
            relation: relation.into(),
            args,
            basis: basis.clone(),
        });
    }

    fn declarations(mut self) -> Vec<FactDecl> {
        self.0.sort();
        self.0.dedup();
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        self.0
            .into_iter()
            .map(
                |Fact {
                     relation,
                     args,
                     basis,
                 }| {
                    let count = counts.entry(relation.clone()).or_default();
                    let id = format!("{relation}_{count}");
                    *count += 1;
                    FactDecl {
                        id,
                        relation,
                        args,
                        confidence: 1.0,
                        provenance: vec![basis.source.clone()],
                        basis: Some(basis),
                        doc: None,
                    }
                },
            )
            .collect()
    }
}
