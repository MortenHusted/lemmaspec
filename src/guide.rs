//! The human guide: one artifact read as an argument, from observations to
//! conclusions, so a reader can follow, challenge, and decide.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::artifact::MutationOperator;
use crate::html::html_escape;
use crate::narrative::{read_with, value_text};
use crate::projection::{GraphNode, GraphNodeData, GraphProjection};

/// Every fact stands somewhere on the ladder from evidence to conclusion.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Standing {
    /// Asserted and backed by provenance.
    Observation,
    /// Asserted without provenance, or below full confidence: a decision point.
    Assumption,
    /// Derived by a rule.
    Conclusion,
}

impl Standing {
    fn label(self) -> &'static str {
        match self {
            Standing::Observation => "observation",
            Standing::Assumption => "assumption",
            Standing::Conclusion => "conclusion",
        }
    }

    fn tone(self) -> &'static str {
        match self {
            Standing::Observation => "c-stable",
            Standing::Assumption => "c-attention",
            Standing::Conclusion => "c-additive",
        }
    }
}

struct Relation<'a> {
    roles: &'a [String],
    reads: Option<&'a str>,
    doc: Option<&'a str>,
}

/// One producing rule and the facts its witness needed.
struct Support<'a> {
    rule: &'a str,
    body: Vec<&'a str>,
}

struct Index<'a> {
    projection: &'a GraphProjection,
    nodes: BTreeMap<&'a str, &'a GraphNode>,
    relations: BTreeMap<&'a str, Relation<'a>>,
    rules_by_name: BTreeMap<&'a str, &'a str>,
    expectations_by_name: BTreeMap<&'a str, &'a str>,
    supports: BTreeMap<&'a str, Vec<Support<'a>>>,
    dependents: BTreeMap<&'a str, Vec<&'a str>>,
    proves: BTreeMap<&'a str, Vec<&'a str>>,
    proven_by: BTreeMap<&'a str, Vec<&'a str>>,
    rule_yield: BTreeMap<&'a str, usize>,
    derived_by: BTreeMap<&'a str, Vec<&'a str>>,
    declaration_ids: BTreeSet<&'a str>,
}

impl<'a> Index<'a> {
    fn build(projection: &'a GraphProjection) -> Self {
        let nodes: BTreeMap<&str, &GraphNode> = projection
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect();
        let mut relations = BTreeMap::new();
        let mut rules_by_name = BTreeMap::new();
        let mut expectations_by_name = BTreeMap::new();
        let mut declaration_ids = BTreeSet::new();
        for node in &projection.nodes {
            match &node.data {
                GraphNodeData::Fact { declarations, .. } => {
                    declaration_ids.extend(declarations.iter().map(String::as_str));
                }
                GraphNodeData::Relation {
                    name,
                    roles,
                    reads,
                    doc,
                    ..
                } => {
                    relations.insert(
                        name.as_str(),
                        Relation {
                            roles,
                            reads: reads.as_deref(),
                            doc: doc.as_deref(),
                        },
                    );
                }
                GraphNodeData::Rule { name, .. } => {
                    rules_by_name.insert(name.as_str(), node.id.as_str());
                }
                GraphNodeData::Expectation { name, .. } => {
                    expectations_by_name.insert(name.as_str(), node.id.as_str());
                }
                _ => {}
            }
        }

        let mut supports: BTreeMap<&str, Vec<Support>> = BTreeMap::new();
        let mut dependents: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        let mut proves: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        let mut proven_by: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        let mut rule_yield: BTreeMap<&str, usize> = BTreeMap::new();
        let mut derived_by: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for edge in &projection.edges {
            let (from, to) = (edge.from.as_str(), edge.to.as_str());
            let node_type = |id: &str| nodes.get(id).map(|node| node.node_type());
            match (edge.rel.as_str(), edge.basis.as_deref()) {
                ("depends_on", Some("proof_witness")) => {
                    let Some(rule) = edge.witness.as_deref() else {
                        continue;
                    };
                    let entry = supports.entry(from).or_default();
                    match entry.iter_mut().find(|support| support.rule == rule) {
                        Some(support) => support.body.push(to),
                        None => entry.push(Support {
                            rule,
                            body: vec![to],
                        }),
                    }
                    dependents.entry(to).or_default().push(from);
                }
                ("proves", _) => {
                    proves.entry(from).or_default().push(to);
                    proven_by.entry(to).or_default().push(from);
                }
                ("derives", Some("proof_witness")) => {
                    *rule_yield.entry(from).or_default() += 1;
                }
                ("derives", Some("declaration")) if node_type(to) == Some("relation") => {
                    derived_by.entry(to).or_default().push(from);
                }
                _ => {}
            }
        }

        Self {
            projection,
            nodes,
            relations,
            rules_by_name,
            expectations_by_name,
            supports,
            dependents,
            proves,
            proven_by,
            rule_yield,
            derived_by,
            declaration_ids,
        }
    }

    /// Provenance the author attached. The engine also seeds every asserted
    /// fact's provenance with its own declaration id and unions those into
    /// derived facts; declaration ids are bookkeeping, not evidence.
    fn evidence(&self, node: &'a GraphNode) -> Vec<&'a str> {
        match &node.data {
            GraphNodeData::Fact { provenance, .. } => provenance
                .iter()
                .map(String::as_str)
                .filter(|source| !self.declaration_ids.contains(source))
                .collect(),
            _ => Vec::new(),
        }
    }

    fn facts(&self) -> impl Iterator<Item = &'a GraphNode> + '_ {
        self.projection
            .nodes
            .iter()
            .filter(|node| matches!(node.data, GraphNodeData::Fact { .. }))
    }

    fn standing(&self, node: &'a GraphNode) -> Standing {
        match &node.data {
            GraphNodeData::Fact {
                origin, confidence, ..
            } => {
                if origin == "derived" {
                    Standing::Conclusion
                } else if self.evidence(node).is_empty() || *confidence < 1.0 {
                    Standing::Assumption
                } else {
                    Standing::Observation
                }
            }
            _ => Standing::Conclusion,
        }
    }

    /// A fact as a sentence: the relation's template when it has one,
    /// otherwise the atom with role names alongside the values.
    fn sentence(&self, node: &GraphNode) -> String {
        let GraphNodeData::Fact {
            relation,
            args,
            reading,
            ..
        } = &node.data
        else {
            return html_escape(&label(node));
        };
        if let Some(reading) = reading {
            return html_escape(reading);
        }
        let texts: Vec<String> = args.iter().map(value_text).collect();
        self.atom_markup(relation, &texts)
    }

    fn atom_markup(&self, relation: &str, args: &[String]) -> String {
        let roles = self
            .relations
            .get(relation)
            .map(|info| info.roles)
            .unwrap_or_default();
        let inner = args
            .iter()
            .enumerate()
            .map(|(position, value)| match roles.get(position) {
                Some(role) => format!("<i>{}</i> {}", html_escape(role), html_escape(value)),
                None => html_escape(value),
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!("<code>{}({inner})</code>", html_escape(relation))
    }

    /// A rule atom read with its variables in place, e.g. `Mutation is
    /// governed by Policy`. Comparisons and anything unparsable stay verbatim.
    fn condition_markup(&self, text: &str) -> String {
        match split_atom(text) {
            Some((negated, predicate, args)) => {
                let roles = self
                    .relations
                    .get(predicate)
                    .map(|info| info.roles)
                    .unwrap_or_default();
                let args: Vec<String> = args
                    .into_iter()
                    .enumerate()
                    .map(
                        |(position, arg)| match (arg.as_str(), roles.get(position)) {
                            ("_", Some(role)) => format!("any {role}"),
                            ("_", None) => "anything".to_string(),
                            _ => arg,
                        },
                    )
                    .collect();
                let body = match self.relations.get(predicate) {
                    Some(Relation {
                        reads: Some(template),
                        roles,
                        ..
                    }) => html_escape(&read_with(template, roles, &args)),
                    _ => self.atom_markup(predicate, &args),
                };
                if negated {
                    format!("<span class=\"not\">not</span> {body}")
                } else {
                    body
                }
            }
            None => format!("<code>{}</code>", html_escape(text)),
        }
    }

    /// Everything that would fall if this fact were withdrawn.
    fn blast_radius(&self, fact_id: &str) -> (usize, usize) {
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::from([fact_id]);
        let mut claims = BTreeSet::new();
        while let Some(current) = queue.pop_front() {
            if let Some(expectations) = self.proves.get(current) {
                claims.extend(expectations.iter().copied());
            }
            for dependent in self.dependents.get(current).into_iter().flatten() {
                if seen.insert(*dependent) {
                    queue.push_back(dependent);
                }
            }
        }
        (seen.len(), claims.len())
    }

    fn link(&self, id: &str) -> String {
        let Some(node) = self.nodes.get(id) else {
            return html_escape(id);
        };
        format!(
            "<a class=\"ref\" href=\"#{}\">{}</a>",
            anchor(id),
            match &node.data {
                GraphNodeData::Fact { .. } => self.sentence(node),
                _ => html_escape(&label(node)),
            }
        )
    }

    fn because(&self, fact_id: &str, depth: usize, path: &mut Vec<String>) -> String {
        let Some(supports) = self.supports.get(fact_id) else {
            return String::new();
        };
        path.push(fact_id.to_string());
        let mut html = String::from("<ul class=\"because\">");
        for support in supports {
            let rule_link = self
                .rules_by_name
                .get(support.rule)
                .map(|id| self.link(id))
                .unwrap_or_else(|| html_escape(support.rule));
            html.push_str(&format!(
                "<li><span class=\"via\">via rule {rule_link}</span><ul>"
            ));
            for body in &support.body {
                let Some(node) = self.nodes.get(body) else {
                    continue;
                };
                let standing = self.standing(node);
                html.push_str(&format!(
                    "<li><span class=\"chip {}\">{}</span> {}",
                    standing.tone(),
                    standing.label(),
                    self.link(body)
                ));
                if standing == Standing::Conclusion
                    && depth < 4
                    && !path.iter().any(|seen| seen == body)
                {
                    let nested = self.because(body, depth + 1, path);
                    if !nested.is_empty() {
                        html.push_str(&format!(
                            "<details><summary>why</summary>{nested}</details>"
                        ));
                    }
                }
                html.push_str("</li>");
            }
            html.push_str("</ul></li>");
        }
        html.push_str("</ul>");
        path.pop();
        html
    }
}

/// Stable element id for a projection node, shared with the graph script.
pub(crate) fn anchor(id: &str) -> String {
    let mut anchor = String::with_capacity(id.len() + 2);
    anchor.push_str("n-");
    anchor.extend(id.chars().map(|character| {
        if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
            character
        } else {
            '-'
        }
    }));
    anchor
}

fn label(node: &GraphNode) -> String {
    match &node.data {
        GraphNodeData::Fact { relation, args, .. } => format!(
            "{relation}({})",
            args.iter().map(value_text).collect::<Vec<_>>().join(", ")
        ),
        GraphNodeData::Symbol { value } => value.clone(),
        GraphNodeData::Spec { name, .. }
        | GraphNodeData::Relation { name, .. }
        | GraphNodeData::Rule { name, .. }
        | GraphNodeData::Mutation { name, .. }
        | GraphNodeData::Expectation { name, .. } => name.clone(),
    }
}

/// `!governed_by(M, P)` → (true, "governed_by", ["M", "P"]).
fn split_atom(text: &str) -> Option<(bool, &str, Vec<String>)> {
    let trimmed = text.trim();
    let (negated, rest) = match trimmed.strip_prefix('!') {
        Some(rest) => (true, rest.trim_start()),
        None => (false, trimmed),
    };
    let open = rest.find('(')?;
    let predicate = rest[..open].trim();
    if predicate.is_empty()
        || !predicate
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        || !rest.ends_with(')')
    {
        return None;
    }
    let inner = &rest[open + 1..rest.len() - 1];
    let mut args = Vec::new();
    let mut depth = 0usize;
    let mut quoted = false;
    let mut current = String::new();
    for character in inner.chars() {
        match character {
            '"' => {
                quoted = !quoted;
                current.push(character);
            }
            '(' if !quoted => {
                depth += 1;
                current.push(character);
            }
            ')' if !quoted => {
                depth = depth.saturating_sub(1);
                current.push(character);
            }
            ',' if !quoted && depth == 0 => {
                args.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(character),
        }
    }
    if !current.trim().is_empty() {
        args.push(current.trim().to_string());
    }
    Some((negated, predicate, args))
}

fn paragraphs(doc: &str) -> String {
    doc.split("\n\n")
        .map(str::trim)
        .filter(|paragraph| !paragraph.is_empty())
        .map(|paragraph| format!("<p>{}</p>", html_escape(paragraph).replace('\n', " ")))
        .collect()
}

fn doc_markup(doc: Option<&str>) -> String {
    doc.map(|doc| format!("<div class=\"doc\">{}</div>", paragraphs(doc)))
        .unwrap_or_default()
}

fn graph_link(id: &str) -> String {
    format!(
        "<a class=\"graph-link\" href=\"#graph\" data-node=\"{}\">show in graph</a>",
        html_escape(id)
    )
}

fn plural(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

fn section(title: &str, intro: &str, body: String) -> String {
    if body.is_empty() {
        return String::new();
    }
    format!(
        "<section class=\"guide\"><h2>{}</h2><p class=\"intro\">{}</p>{body}</section>",
        html_escape(title),
        html_escape(intro)
    )
}

fn fact_card(index: &Index, node: &GraphNode, extra: &str) -> String {
    let GraphNodeData::Fact {
        relation,
        confidence,
        doc,
        ..
    } = &node.data
    else {
        return String::new();
    };
    let standing = index.standing(node);
    let sources = index.evidence(node);
    let mut meta = vec![format!("<code>{}</code>", html_escape(relation))];
    if !sources.is_empty() {
        meta.push(format!(
            "evidence: {}",
            sources
                .iter()
                .map(|source| html_escape(source))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if *confidence < 1.0 {
        meta.push(format!("{:.0}% confidence", confidence * 100.0));
    }
    format!(
        "<article class=\"card\" id=\"{}\" data-standing=\"{}\"><div class=\"card-head\"><span class=\"chip {}\">{}</span><span class=\"sentence\">{}</span>{}</div><div class=\"meta\">{}</div>{}{extra}</article>",
        anchor(&node.id),
        standing.label(),
        standing.tone(),
        standing.label(),
        index.sentence(node),
        graph_link(&node.id),
        meta.join(" · "),
        doc_markup(doc.as_deref())
    )
}

/// Facts grouped under the concept they describe, collapsed when a group is
/// too long to scan.
fn grouped_facts(index: &Index, facts: &[&GraphNode]) -> String {
    let mut groups: BTreeMap<&str, Vec<&GraphNode>> = BTreeMap::new();
    for node in facts {
        if let GraphNodeData::Fact { relation, .. } = &node.data {
            groups.entry(relation.as_str()).or_default().push(node);
        }
    }
    groups
        .into_iter()
        .map(|(relation, nodes)| {
            let doc = index
                .relations
                .get(relation)
                .and_then(|info| info.doc)
                .map(|doc| format!("<p class=\"group-doc\">{}</p>", html_escape(doc)))
                .unwrap_or_default();
            let cards: String = nodes
                .iter()
                .map(|node| fact_card(index, node, ""))
                .collect();
            format!(
                "<details class=\"group\"{}><summary><strong>{}</strong> <span class=\"count\">{}</span></summary>{doc}{cards}</details>",
                if nodes.len() <= 12 { " open" } else { "" },
                html_escape(relation),
                plural(nodes.len(), "fact", "facts")
            )
        })
        .collect()
}

pub struct GuideSections {
    pub question: String,
    pub observations: String,
    pub assumptions: String,
    pub relationships: String,
    pub reasoning: String,
    pub conclusions: String,
    pub claims: String,
    pub stress_tests: String,
}

pub fn render_guide(projection: &GraphProjection) -> GuideSections {
    let index = Index::build(projection);
    GuideSections {
        question: render_question(&index),
        observations: render_observations(&index),
        assumptions: render_assumptions(&index),
        relationships: render_relationships(&index),
        reasoning: render_reasoning(&index),
        conclusions: render_conclusions(&index),
        claims: render_claims(&index),
        stress_tests: render_stress_tests(&index),
    }
}

fn render_question(index: &Index) -> String {
    index
        .projection
        .nodes
        .iter()
        .find_map(|node| match &node.data {
            GraphNodeData::Spec { doc: Some(doc), .. } => Some(format!(
                "<section class=\"question\"><h2>The question</h2>{}</section>",
                paragraphs(doc)
            )),
            _ => None,
        })
        .unwrap_or_default()
}

fn render_observations(index: &Index) -> String {
    let facts: Vec<&GraphNode> = index
        .facts()
        .filter(|node| index.standing(node) == Standing::Observation)
        .collect();
    section(
        &format!("Observations ({})", facts.len()),
        "Asserted facts with evidence attached. These are the ground the rest of the argument stands on.",
        grouped_facts(index, &facts),
    )
}

fn render_assumptions(index: &Index) -> String {
    let mut facts: Vec<(&GraphNode, (usize, usize))> = index
        .facts()
        .filter(|node| index.standing(node) == Standing::Assumption)
        .map(|node| (node, index.blast_radius(&node.id)))
        .collect();
    facts.sort_by(|left, right| {
        right
            .1
             .1
            .cmp(&left.1 .1)
            .then(right.1 .0.cmp(&left.1 .0))
            .then_with(|| left.0.id.cmp(&right.0.id))
    });
    let cards: String = facts
        .iter()
        .map(|(node, (conclusions, claims))| {
            let radius = if *conclusions == 0 && *claims == 0 {
                "<div class=\"radius quiet\">Nothing derived rests on this yet.</div>".to_string()
            } else {
                format!(
                    "<div class=\"radius\">If this is wrong, {} and {} fall with it.</div>",
                    plural(*conclusions, "conclusion", "conclusions"),
                    plural(*claims, "claim", "claims")
                )
            };
            fact_card(index, node, &radius)
        })
        .collect();
    section(
        &format!("Assumptions ({})", facts.len()),
        "Asserted without evidence, or below full confidence. Each one is a decision waiting to be made; they are ordered by how much depends on them.",
        cards,
    )
}

fn render_relationships(index: &Index) -> String {
    let mut asserted: BTreeMap<&str, usize> = BTreeMap::new();
    let mut derived: BTreeMap<&str, usize> = BTreeMap::new();
    for node in index.facts() {
        if let GraphNodeData::Fact {
            relation, origin, ..
        } = &node.data
        {
            let counts = if origin == "asserted" {
                &mut asserted
            } else {
                &mut derived
            };
            *counts.entry(relation.as_str()).or_default() += 1;
        }
    }

    let mut concepts = String::new();
    for node in &index.projection.nodes {
        let GraphNodeData::Relation {
            name,
            args,
            roles,
            reads,
            doc,
        } = &node.data
        else {
            continue;
        };
        let signature = if roles.is_empty() {
            format!(
                "{} {}",
                args.len(),
                if args.len() == 1 {
                    "argument"
                } else {
                    "arguments"
                }
            )
        } else {
            roles
                .iter()
                .map(|role| format!("<i>{}</i>", html_escape(role)))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let mut facts = Vec::new();
        if let Some(count) = asserted.get(name.as_str()) {
            facts.push(format!("{count} asserted"));
        }
        if let Some(count) = derived.get(name.as_str()) {
            facts.push(format!("{count} derived"));
        }
        let producers = index
            .derived_by
            .get(node.id.as_str())
            .map(|rules| {
                format!(
                    "concluded by {}",
                    rules
                        .iter()
                        .map(|rule| index.link(rule))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
            .unwrap_or_else(|| "asserted directly".to_string());
        let reads = reads
            .as_deref()
            .map(|template| {
                format!(
                    "<div class=\"reads\">reads as “{}”</div>",
                    html_escape(template)
                )
            })
            .unwrap_or_default();
        concepts.push_str(&format!(
            "<article class=\"card concept\" id=\"{}\"><div class=\"card-head\"><strong>{}</strong><span class=\"signature\">{signature}</span>{}</div><div class=\"meta\">{}{}</div>{reads}{}</article>",
            anchor(&node.id),
            html_escape(name),
            graph_link(&node.id),
            producers,
            if facts.is_empty() {
                String::new()
            } else {
                format!(" · {}", facts.join(", "))
            },
            doc_markup(doc.as_deref())
        ));
    }

    let vocabulary = render_vocabulary(index);
    section(
        "Relationships",
        "The concepts this spec talks about, who concludes them, and the vocabulary they share.",
        format!("{concepts}{vocabulary}"),
    )
}

/// Symbols grouped by the role they play. A role name shared by several
/// relations unifies the vocabulary across them.
fn render_vocabulary(index: &Index) -> String {
    let mut by_role: BTreeMap<String, BTreeMap<&str, usize>> = BTreeMap::new();
    for edge in &index.projection.edges {
        if edge.rel != "references_symbol" || edge.basis.as_deref() != Some("fact_argument") {
            continue;
        }
        let (Some(fact), Some(symbol), Some(position)) = (
            index.nodes.get(edge.from.as_str()),
            index.nodes.get(edge.to.as_str()),
            edge.position,
        ) else {
            continue;
        };
        let (GraphNodeData::Fact { relation, .. }, GraphNodeData::Symbol { value }) =
            (&fact.data, &symbol.data)
        else {
            continue;
        };
        let role = index
            .relations
            .get(relation.as_str())
            .and_then(|info| info.roles.get(position))
            .cloned()
            .unwrap_or_else(|| format!("{relation} · argument {}", position + 1));
        *by_role
            .entry(role)
            .or_default()
            .entry(value.as_str())
            .or_default() += 1;
    }
    if by_role.is_empty() {
        return String::new();
    }
    let groups: String = by_role
        .into_iter()
        .map(|(role, symbols)| {
            let items: String = symbols
                .iter()
                .map(|(symbol, uses)| {
                    format!(
                        "<li><code>{}</code><span class=\"count\">{}</span></li>",
                        html_escape(symbol),
                        plural(*uses, "use", "uses")
                    )
                })
                .collect();
            format!(
                "<details class=\"vocab\"{}><summary><i>{}</i> <span class=\"count\">{}</span></summary><ul>{items}</ul></details>",
                if symbols.len() <= 8 { " open" } else { "" },
                html_escape(&role),
                plural(symbols.len(), "value", "values")
            )
        })
        .collect();
    format!("<h3>Vocabulary</h3><div class=\"vocab-grid\">{groups}</div>")
}

fn render_reasoning(index: &Index) -> String {
    let cards: String = index
        .projection
        .nodes
        .iter()
        .filter_map(|node| {
            let GraphNodeData::Rule {
                name,
                derive,
                when,
                condition_ids,
                doc,
            } = &node.data
            else {
                return None;
            };
            let conditions: String = when
                .iter()
                .enumerate()
                .map(|(position, condition)| {
                    let id = condition_ids
                        .get(position)
                        .map(|id| format!("<span class=\"cond\">{}</span>", html_escape(id)))
                        .unwrap_or_default();
                    format!("<li>{id}{}</li>", index.condition_markup(condition))
                })
                .collect();
            let yielded = index
                .rule_yield
                .get(node.id.as_str())
                .map(|count| plural(*count, "conclusion", "conclusions"))
                .unwrap_or_else(|| "no conclusions so far".to_string());
            Some(format!(
                "<article class=\"card rule\" id=\"{}\"><div class=\"card-head\"><span class=\"chip c-attention\">rule</span><strong>{}</strong><span class=\"count\">{yielded}</span>{}</div><p class=\"lead\">Concludes <span class=\"sentence\">{}</span> when</p><ol class=\"conditions\">{conditions}</ol>{}</article>",
                anchor(&node.id),
                html_escape(name),
                graph_link(&node.id),
                index.condition_markup(derive),
                doc_markup(doc.as_deref())
            ))
        })
        .collect();
    section(
        "Reasoning",
        "The rules that turn observations and assumptions into conclusions. Read each one as: this follows, whenever all of these hold.",
        cards,
    )
}

fn render_conclusions(index: &Index) -> String {
    let facts: Vec<&GraphNode> = index
        .facts()
        .filter(|node| index.standing(node) == Standing::Conclusion)
        .collect();
    let mut groups: BTreeMap<&str, Vec<&GraphNode>> = BTreeMap::new();
    for node in &facts {
        if let GraphNodeData::Fact { relation, .. } = &node.data {
            groups.entry(relation.as_str()).or_default().push(node);
        }
    }
    let body: String = groups
        .into_iter()
        .map(|(relation, nodes)| {
            let cards: String = nodes
                .iter()
                .map(|node| {
                    let because = index.because(&node.id, 0, &mut Vec::new());
                    let because = if because.is_empty() {
                        String::new()
                    } else {
                        format!("<div class=\"why\"><span class=\"why-label\">because</span>{because}</div>")
                    };
                    fact_card(index, node, &because)
                })
                .collect();
            format!(
                "<details class=\"group\"{}><summary><strong>{}</strong> <span class=\"count\">{}</span></summary>{cards}</details>",
                if nodes.len() <= 12 { " open" } else { "" },
                html_escape(relation),
                plural(nodes.len(), "conclusion", "conclusions")
            )
        })
        .collect();
    section(
        &format!("Conclusions ({})", facts.len()),
        "What follows from the above. Every conclusion unfolds into the rule and the facts that produced it, down to the observations.",
        body,
    )
}

fn render_claims(index: &Index) -> String {
    let mut expectations: Vec<&GraphNode> = index
        .projection
        .nodes
        .iter()
        .filter(|node| matches!(node.data, GraphNodeData::Expectation { .. }))
        .collect();
    expectations.sort_by_key(|node| match &node.data {
        GraphNodeData::Expectation { satisfied, .. } => *satisfied,
        _ => true,
    });
    let cards: String = expectations
        .iter()
        .filter_map(|node| {
            let GraphNodeData::Expectation {
                name,
                query,
                expected_count,
                actual_count,
                satisfied,
                doc,
            } = &node.data
            else {
                return None;
            };
            let (tone, verdict) = if *satisfied {
                ("c-stable", format!("Found {actual_count}. Confirmed."))
            } else {
                (
                    "c-critical",
                    format!("Found {actual_count}. This claim is open until the facts or the claim change."),
                )
            };
            let proof = index
                .proven_by
                .get(node.id.as_str())
                .map(|facts| {
                    let shown: Vec<String> = facts.iter().take(8).map(|fact| format!("<li>{}</li>", index.link(fact))).collect();
                    let more = if facts.len() > 8 {
                        format!("<li class=\"more\">and {} more</li>", facts.len() - 8)
                    } else {
                        String::new()
                    };
                    format!(
                        "<div class=\"why\"><span class=\"why-label\">supported by</span><ul class=\"because\">{}{more}</ul></div>",
                        shown.join("")
                    )
                })
                .unwrap_or_default();
            Some(format!(
                "<article class=\"card claim\" id=\"{}\" data-status=\"{}\"><div class=\"card-head\"><span class=\"chip {tone}\">{}</span><strong>{}</strong>{}</div><p class=\"lead\">There must be exactly {} where <span class=\"sentence\">{}</span>.</p><p class=\"verdict\">{verdict}</p>{}{proof}</article>",
                anchor(&node.id),
                if *satisfied { "passed" } else { "failed" },
                if *satisfied { "confirmed" } else { "open" },
                html_escape(name),
                graph_link(&node.id),
                plural(*expected_count, "result", "results"),
                index.condition_markup(query),
                doc_markup(doc.as_deref())
            ))
        })
        .collect();
    section(
        "Claims",
        "The acceptance criteria this spec commits to, checked against the walk. Open claims come first.",
        cards,
    )
}

fn render_stress_tests(index: &Index) -> String {
    let cards: String = index
        .projection
        .nodes
        .iter()
        .filter_map(|node| {
            let GraphNodeData::Mutation {
                name,
                operator,
                relation,
                except,
                must_fail,
                doc,
            } = &node.data
            else {
                return None;
            };
            let target = match operator {
                MutationOperator::DropRule => "Dropping any rule".to_string(),
                MutationOperator::DropCondition => "Dropping any single rule condition".to_string(),
                MutationOperator::DropFact => format!(
                    "Dropping any <code>{}</code> fact",
                    html_escape(relation.as_deref().unwrap_or("?"))
                ),
            };
            let exceptions = if except.is_empty() {
                String::new()
            } else {
                format!(
                    " except {}",
                    except
                        .iter()
                        .map(|item| format!("<code>{}</code>", html_escape(item)))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            let oracle = match must_fail {
                Some(expectation) => format!(
                    "must break the claim {}",
                    index
                        .expectations_by_name
                        .get(expectation.as_str())
                        .map(|id| index.link(id))
                        .unwrap_or_else(|| html_escape(expectation))
                ),
                None => "must break at least one claim".to_string(),
            };
            Some(format!(
                "<article class=\"card policy\" id=\"{}\"><div class=\"card-head\"><span class=\"chip c-attention\">stress test</span><strong>{}</strong>{}</div><p class=\"lead\">{target}{exceptions} {oracle}.</p>{}</article>",
                anchor(&node.id),
                html_escape(name),
                graph_link(&node.id),
                doc_markup(doc.as_deref())
            ))
        })
        .collect();
    section(
        "Stress tests",
        "Mutation policies: which parts of the reasoning must be load-bearing. Run lemmaspec mutate to see which mutants survive.",
        cards,
    )
}
