//! Optional consumer of Markdown plans. This crate never executes project commands.

use std::collections::{BTreeMap, BTreeSet};

use lemmaspec::{Artifact, ExpectationDecl, FactDecl, FactValue, SymbolDecl};
use sha2::{Digest, Sha256};

pub const CHECKER: &str = include_str!("../plan_checker.lemmaspec");

#[derive(Default)]
struct Unit {
    id: String,
    source: String,
    requirements: Option<Vec<String>>,
    dependencies: Option<Vec<String>>,
}

/// Project the documented `ce-unified-plan/v1` Markdown subset into evidence.
/// The caller controls the source path recorded in provenance. No files are read.
/// The returned artifact can be extended with identity-bound status facts before
/// printing with `lemmaspec::print_artifact` and binding to [`CHECKER`].
pub fn project_plan(markdown: &str, source: &str) -> Result<Artifact, String> {
    if source.is_empty() || source.contains(['#', '\n', '\r']) {
        return Err("source must be a nonempty path without a fragment or newline".into());
    }
    let mut artifact = Artifact {
        name: "plan_evidence".into(),
        doc: Some("Does this plan cover its requirements and preserve its declared structure? Structural checks do not establish completion.".into()),
        notes: Some("Projected by the lemmaspec-plan ce-unified-plan/v1 subset documented in the consumer README. Structural checks assert coverage, not acceptance.".into()),
        symbols: Vec::new(),
        relations: Vec::new(),
        facts: Vec::new(),
        rules: Vec::new(),
        expectations: Vec::new(),
        mutations: Vec::new(),
    };
    let mut lines = markdown.lines().enumerate();
    if lines.next().map(|(_, line)| line) != Some("---") {
        return Err("plan must start with YAML front matter".into());
    }
    let mut contract = false;
    let mut closed = false;
    for (_, line) in lines.by_ref() {
        if line == "---" {
            closed = true;
            break;
        }
        if line.starts_with("artifact_contract:") {
            if contract || line.trim() != "artifact_contract: ce-unified-plan/v1" {
                return Err("expected one artifact_contract: ce-unified-plan/v1".into());
            }
            contract = true;
        }
    }
    if !closed || !contract {
        return Err("missing ce-unified-plan/v1 front matter".into());
    }

    let mut headings = BTreeSet::new();
    let mut anchor = String::new();
    let mut section = String::new();
    let mut section_level = 0;
    let mut title = false;
    let mut units: Vec<Unit> = Vec::new();
    let mut current_unit = None;
    let mut requirement_ids = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut seen_sections = BTreeSet::new();
    let mut section_items = 0;
    let mut fence = None;

    for (index, raw) in lines {
        let line = raw.trim();
        let error = |message: String| format!("line {}: {message}", index + 1);
        if let Some((delimiter, length)) = fence {
            if fence_marker(line).is_some_and(|(mark, size)| {
                mark == delimiter && size >= length && line[size..].trim().is_empty()
            }) {
                fence = None;
            }
            continue;
        }
        if let Some(marker) = fence_marker(line) {
            fence = Some(marker);
            continue;
        }
        if let Some((level, text)) = heading(line) {
            if !section.is_empty() && level <= section_level {
                if section_items == 0 {
                    return Err(error(format!("{section} requires items or '- None'")));
                }
                section.clear();
            }
            if level <= 3 {
                current_unit = None;
            }
            anchor = heading_anchor(text);
            if anchor.is_empty() || !headings.insert(anchor.clone()) {
                return Err(error(
                    "empty or duplicate heading anchor is unsupported".into(),
                ));
            }
            if level == 1 {
                if title {
                    return Err(error("only one level-one plan title is supported".into()));
                }
                title = true;
                add_symbol(&mut artifact, "plan", text, &format!("{source}#{anchor}"));
                add_fact(
                    &mut artifact,
                    "plan",
                    &["plan"],
                    &format!("{source}#{anchor}"),
                );
            }
            if text.starts_with('U') && text.as_bytes().get(1).is_some_and(u8::is_ascii_digit) {
                if level != 3 {
                    return Err(error("units must use level-three headings".into()));
                }
                let (id, label) = parse_unit_heading(text).map_err(error)?;
                if !ids.insert(id.clone()) {
                    return Err(error(format!("duplicate id {id}")));
                }
                let location = format!("{source}#{anchor}");
                add_symbol(&mut artifact, &id, label, &location);
                add_fact(&mut artifact, "scheduled", &[&id], &location);
                add_fact(&mut artifact, "plan_item", &["plan", &id], &location);
                units.push(Unit {
                    id,
                    source: location,
                    ..Unit::default()
                });
                current_unit = Some(units.len() - 1);
            }
            if ["Requirements", "Gates", "Deferred", "Semantic inputs"].contains(&text) {
                if !section.is_empty() {
                    return Err(error("nested structural sections are unsupported".into()));
                }
                if !seen_sections.insert(text.to_string()) {
                    return Err(error(format!("duplicate {text} section")));
                }
                section = text.to_string();
                section_level = level;
                section_items = 0;
            } else if !section.is_empty() {
                return Err(error(
                    "nested headings in structural lists are unsupported".into(),
                ));
            }
            continue;
        }
        if line.is_empty() {
            continue;
        }
        if line.contains("Requirements:") || line.contains("Dependencies:") {
            let unit = current_unit.ok_or_else(|| error("unit fields outside a unit".into()))?;
            parse_unit_fields(line, &mut units[unit]).map_err(error)?;
            continue;
        }
        if !section.is_empty() {
            if let Some(label) = line
                .strip_prefix("**")
                .and_then(|text| text.strip_suffix("**"))
                .filter(|text| !text.is_empty() && !text.contains("**"))
            {
                if looks_like_declaration(label) {
                    return Err(error(
                        "bold-wrapped structural declarations are unsupported".into(),
                    ));
                }
                continue;
            }
            let item = line.strip_prefix("- ").ok_or_else(|| {
                error(format!("{section} accepts only '- ID. Label' or '- None'"))
            })?;
            if item == "None" {
                if section_items != 0 || section == "Requirements" {
                    return Err(error(
                        "None must be the sole item of a non-requirement section".into(),
                    ));
                }
                section_items = usize::MAX;
                continue;
            }
            if section_items == usize::MAX {
                return Err(error("items cannot follow None".into()));
            }
            let (prefix, relation) = match section.as_str() {
                "Requirements" => ('R', "requirement"),
                "Gates" => ('G', "gate"),
                "Deferred" => ('D', "deferred"),
                "Semantic inputs" => ('S', "semantic_input"),
                _ => unreachable!(),
            };
            let (id, label) = match prefix {
                'G' => parse_gate(item),
                'D' => parse_deferred_shape(item),
                'S' => parse_semantic_input(item),
                'R' => declaration(item, 'R'),
                _ => unreachable!(),
            }
            .map_err(error)?;
            if !ids.insert(id.clone()) {
                return Err(error(format!("duplicate id {id}")));
            }
            if prefix == 'R' {
                requirement_ids.insert(id.clone());
            }
            let location = format!("{source}#{anchor}");
            add_symbol(&mut artifact, &id, label, &location);
            add_fact(&mut artifact, relation, &[&id], &location);
            if prefix == 'R' || prefix == 'G' {
                add_fact(&mut artifact, "plan_item", &["plan", &id], &location);
            }
            section_items += 1;
            continue;
        }
        if line.starts_with('-') && line.get(2..).is_some_and(looks_like_declaration) {
            return Err(error(
                "structural declaration outside its named section".into(),
            ));
        }
        // Narrative outside structural lists is intentionally not interpreted.
    }
    if fence.is_some() {
        return Err("unterminated fenced code block".into());
    }
    if !section.is_empty() && section_items == 0 {
        return Err(format!("{section} requires items or '- None'"));
    }
    if !title || units.is_empty() || requirement_ids.is_empty() {
        return Err("a plan needs a title, at least one unit, and at least one requirement".into());
    }
    for required in ["Requirements", "Gates", "Deferred", "Semantic inputs"] {
        if !seen_sections.contains(required) {
            return Err(format!(
                "missing {required} section (use '- None' when empty)"
            ));
        }
    }
    let unit_ids: BTreeSet<_> = units.iter().map(|u| u.id.as_str()).collect();
    for unit in &units {
        let requirements = unit.requirements.as_ref().ok_or_else(|| {
            format!(
                "{} needs an explicit Requirements field (None allowed)",
                unit.id
            )
        })?;
        let dependencies = unit.dependencies.as_ref().ok_or_else(|| {
            format!(
                "{} needs an explicit Dependencies field (None allowed)",
                unit.id
            )
        })?;
        for requirement in requirements {
            if !requirement_ids.contains(requirement) {
                return Err(format!(
                    "{} references unknown requirement {requirement}",
                    unit.id
                ));
            }
            add_fact(
                &mut artifact,
                "covers",
                &[&unit.id, requirement],
                &unit.source,
            );
        }
        for dependency in dependencies {
            if !unit_ids.contains(dependency.as_str()) {
                return Err(format!(
                    "{} references unknown dependency {dependency}",
                    unit.id
                ));
            }
            if dependency == &unit.id {
                return Err(format!("{} depends on itself", unit.id));
            }
            add_fact(
                &mut artifact,
                "depends_on",
                &[&unit.id, dependency],
                &unit.source,
            );
        }
    }
    reject_dependency_cycles(&units)?;
    // These fixed policies must fail when a projection leaves coverage missing.
    expectation(
        &mut artifact,
        "all_requirements_covered",
        "uncovered(Requirement)",
        0,
    );
    expectation(
        &mut artifact,
        "all_units_cover_requirements",
        "uncovered_unit(Unit)",
        0,
    );
    // Explicit rows guard structural preservation, without asserting readiness or
    // computing an expected count from the checker's own answers.
    for unit in &units {
        expectation(
            &mut artifact,
            &format!("scheduled_{}", unit.id),
            &format!("scheduled({})", unit.id),
            1,
        );
        for dependency in unit.dependencies.as_ref().unwrap() {
            expectation(
                &mut artifact,
                &format!("dependency_{}_{}", unit.id, dependency),
                &format!("depends_on({}, {dependency})", unit.id),
                1,
            );
        }
    }
    for requirement in requirement_ids {
        expectation(
            &mut artifact,
            &format!("covered_{requirement}"),
            &format!("covered({requirement})"),
            1,
        );
    }
    let items: Vec<_> = artifact
        .facts
        .iter()
        .filter(|f| ["deferred", "semantic_input", "gate"].contains(&f.relation.as_str()))
        .map(|f| {
            (
                f.relation.clone(),
                match &f.args[0] {
                    FactValue::Symbol(s) => s.clone(),
                    _ => unreachable!(),
                },
            )
        })
        .collect();
    for (relation, item) in items {
        let query_relation = if relation == "deferred" {
            "omitted"
        } else {
            &relation
        };
        expectation(
            &mut artifact,
            &format!("preserved_{relation}_{item}"),
            &format!("{query_relation}({item})"),
            1,
        );
    }
    let identity = format!("sha256:{:x}", Sha256::digest(markdown.as_bytes()));
    for fact in &mut artifact.facts {
        fact.basis = Some(lemmaspec::artifact::EvidenceBasis {
            kind: lemmaspec::artifact::EvidenceKind::Snapshot,
            source: fact.provenance[0].clone(),
            identity: Some(identity.clone()),
        });
    }
    artifact.facts.sort_by(|a, b| a.id.cmp(&b.id));
    artifact.symbols.sort_by(|a, b| a.value.cmp(&b.value));
    artifact.expectations.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(artifact)
}

fn heading(line: &str) -> Option<(usize, &str)> {
    let level = line.bytes().take_while(|b| *b == b'#').count();
    if (1..=6).contains(&level) {
        line[level..].strip_prefix(' ').map(|text| (level, text))
    } else {
        None
    }
}

fn fence_marker(line: &str) -> Option<(u8, usize)> {
    let delimiter = *line.as_bytes().first()?;
    if delimiter != b'`' && delimiter != b'~' {
        return None;
    }
    let length = line.bytes().take_while(|byte| *byte == delimiter).count();
    (length >= 3).then_some((delimiter, length))
}

/// Anchors use lowercase alphanumerics, `_`, `-`, and spaces converted to `-`.
/// Duplicate anchors are rejected rather than relying on renderer suffix rules.
pub fn heading_anchor(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter_map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                Some(c)
            } else if c == ' ' {
                Some('-')
            } else {
                None
            }
        })
        .collect()
}

fn split_id(id: &str, family: char) -> Result<(&str, u32), String> {
    let at = id
        .find(|c: char| c.is_ascii_digit())
        .ok_or_else(|| format!("invalid id {id:?}"))?;
    let (prefix, number) = id.split_at(at);
    if !prefix.starts_with(family)
        || !prefix.chars().all(|c| c.is_ascii_uppercase())
        || (family != 'R' && prefix.len() != 1)
        || number.starts_with('0')
        || !number.chars().all(|c| c.is_ascii_digit())
    {
        return Err(format!("invalid {family} id {id:?}"));
    }
    let value = number
        .parse::<u32>()
        .map_err(|_| format!("invalid id number {id:?}"))?;
    if value > 10000 {
        return Err("id numbers must be 1..10000".into());
    }
    Ok((prefix, value))
}

fn declaration(text: &str, family: char) -> Result<(String, &str), String> {
    let (id, label) = text
        .split_once(". ")
        .ok_or_else(|| format!("expected {family}1. Label"))?;
    split_id(id, family)?;
    if label.trim().is_empty() || label.trim() != label {
        return Err("labels must be nonempty and trimmed".into());
    }
    Ok((id.to_ascii_lowercase(), label))
}

fn parse_unit_heading(text: &str) -> Result<(String, &str), String> {
    declaration(text, 'U')
}

fn parse_gate(text: &str) -> Result<(String, &str), String> {
    declaration(text, 'G')
}

fn parse_deferred_shape(text: &str) -> Result<(String, &str), String> {
    declaration(text, 'D')
}

fn parse_semantic_input(text: &str) -> Result<(String, &str), String> {
    declaration(text, 'S')
}

fn looks_like_declaration(text: &str) -> bool {
    text.chars().next().is_some_and(|c| "RUGDS".contains(c))
        && text
            .split_whitespace()
            .next()
            .is_some_and(|id| id.chars().any(|c| c.is_ascii_digit()))
}

fn parse_unit_fields(mut text: &str, unit: &mut Unit) -> Result<(), String> {
    while !text.is_empty() {
        let (kind, rest) = if let Some(rest) = text.strip_prefix("**Requirements:**") {
            ('R', rest)
        } else if let Some(rest) = text.strip_prefix("**Dependencies:**") {
            ('U', rest)
        } else {
            return Err(format!("malformed unit structural field: {text}"));
        };
        let end = rest.find("**").unwrap_or(rest.len());
        let value = rest[..end].trim().trim_end_matches('.');
        let refs = parse_references(value, kind)?;
        let field = if kind == 'R' {
            &mut unit.requirements
        } else {
            &mut unit.dependencies
        };
        if field.replace(refs).is_some() {
            return Err("duplicate unit structural field".into());
        }
        text = rest[end..].trim();
    }
    Ok(())
}

/// Expand comma-separated IDs and ascending, same-prefix inclusive ranges.
pub fn parse_references(text: &str, family: char) -> Result<Vec<String>, String> {
    if text == "None" {
        return Ok(Vec::new());
    }
    let mut ids = BTreeSet::new();
    for item in text.split(',').map(str::trim) {
        if let Some((first, last)) = item.split_once('-') {
            let (prefix, start) = split_id(first, family)?;
            let (end_prefix, end) = split_id(last, family)?;
            if family != 'R' || prefix != end_prefix || start >= end {
                return Err(format!("invalid range {item}"));
            }
            for n in start..=end {
                if !ids.insert(format!("{}{n}", prefix.to_ascii_lowercase())) {
                    return Err(format!("duplicate reference in {text}"));
                }
            }
        } else {
            split_id(item, family)?;
            if !ids.insert(item.to_ascii_lowercase()) {
                return Err(format!("duplicate reference {item}"));
            }
        }
    }
    Ok(ids.into_iter().collect())
}

fn reject_dependency_cycles(units: &[Unit]) -> Result<(), String> {
    let mut pending: BTreeMap<_, BTreeSet<_>> = units
        .iter()
        .map(|u| {
            (
                u.id.as_str(),
                u.dependencies
                    .as_ref()
                    .unwrap()
                    .iter()
                    .map(String::as_str)
                    .collect(),
            )
        })
        .collect();
    while !pending.is_empty() {
        let ready: BTreeSet<_> = pending
            .iter()
            .filter(|(_, deps)| deps.is_empty())
            .map(|(id, _)| *id)
            .collect();
        if ready.is_empty() {
            return Err("dependency cycle is unsupported".into());
        }
        pending.retain(|id, _| !ready.contains(id));
        for deps in pending.values_mut() {
            deps.retain(|id| !ready.contains(id));
        }
    }
    Ok(())
}

fn add_symbol(artifact: &mut Artifact, id: &str, label: &str, source: &str) {
    artifact.symbols.push(SymbolDecl {
        value: id.into(),
        label: label.into(),
        source: Some(source.into()),
        doc: None,
    });
}

fn add_fact(artifact: &mut Artifact, relation: &str, args: &[&str], source: &str) {
    artifact.facts.push(FactDecl {
        id: format!("{relation}_{}", args.join("_")),
        relation: relation.into(),
        args: args
            .iter()
            .map(|arg| FactValue::Symbol((*arg).into()))
            .collect(),
        confidence: 1.0,
        provenance: vec![source.into()],
        basis: None,
        doc: None,
    });
}

fn expectation(artifact: &mut Artifact, id: &str, query: &str, count: usize) {
    artifact.expectations.push(ExpectationDecl {
        id: id.into(),
        query: query.into(),
        count,
        doc: None,
    });
}

/// Add a status evidence artifact without allowing it to redefine plan structure
/// or checker rules. A status declaration is producer-supplied evidence, not a
/// certificate: the caller remains responsible for observing the claimed state.
pub fn compose_status(mut plan: Artifact, status: Artifact) -> Result<Artifact, String> {
    if !status.relations.is_empty() || !status.rules.is_empty() || !status.mutations.is_empty() {
        return Err("status may contain only facts, expectations, labels, and notes".into());
    }
    let owners: BTreeSet<String> = plan
        .facts
        .iter()
        .filter(|fact| fact.relation == "scheduled" || fact.relation == "gate")
        .filter_map(|fact| match fact.args.as_slice() {
            [FactValue::Symbol(id)] => Some(id.clone()),
            _ => None,
        })
        .collect();
    let mut claims = BTreeMap::new();
    let mut targets = 0;
    for fact in &status.facts {
        match (fact.relation.as_str(), fact.args.as_slice()) {
            ("acceptance_claim", [FactValue::Symbol(claim), FactValue::Symbol(owner)]) => {
                if !owners.contains(owner) {
                    return Err(format!(
                        "acceptance claim references unknown unit or gate {owner}"
                    ));
                }
                if claims.insert(claim.clone(), owner.clone()).is_some() {
                    return Err(format!("duplicate acceptance claim {claim}"));
                }
            }
            ("observed", [FactValue::Symbol(_), FactValue::Symbol(_)]) => {}
            ("target_identity", [FactValue::Symbol(_)]) => {
                targets += 1;
            }
            _ => {
                return Err(format!(
                    "unsupported status fact {} or invalid arguments",
                    fact.relation
                ))
            }
        }
    }
    if targets != 1 {
        return Err("status requires exactly one target_identity fact".into());
    }
    // Acceptance is an explicit policy query, never an inferred ready count.
    // Install these fixed expectations even if a status producer omitted them.
    for (id, query) in [
        ("status_acceptance_policy", "accepted(plan)"),
        ("status_single_target", "target_identity(Identity)"),
    ] {
        merge_named(
            &mut plan.expectations,
            ExpectationDecl {
                id: id.into(),
                query: query.into(),
                count: 1,
                doc: None,
            },
            |e| &e.id,
        )?;
    }
    for fact in status.facts {
        merge_named(&mut plan.facts, fact, |f| &f.id)?;
    }
    for symbol in status.symbols {
        merge_named(&mut plan.symbols, symbol, |s| &s.value)?;
    }
    for expected in status.expectations {
        merge_named(&mut plan.expectations, expected, |e| &e.id)?;
    }
    for text in [status.doc, status.notes].into_iter().flatten() {
        plan.notes = Some(match plan.notes {
            Some(notes) => format!("{notes}\n\n{text}"),
            None => text,
        });
    }
    plan.facts.sort_by(|a, b| a.id.cmp(&b.id));
    plan.symbols.sort_by(|a, b| a.value.cmp(&b.value));
    plan.expectations.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(plan)
}

fn merge_named<T: PartialEq>(
    items: &mut Vec<T>,
    item: T,
    name: impl Fn(&T) -> &str,
) -> Result<(), String> {
    if let Some(previous) = items.iter().find(|previous| name(previous) == name(&item)) {
        if previous != &item {
            return Err(format!("conflicting declaration {}", name(&item)));
        }
    } else {
        items.push(item);
    }
    Ok(())
}
