//! What an agent needs from the binary itself: an orientation it can read
//! before touching a spec, and the skill this binary was built with,
//! installed next to the project it works in so skill and CLI never drift.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const SKILL: &str = include_str!("../plugins/lemmaspec/skills/lemmaspec/SKILL.md");
pub const OPENAI_AGENT: &str =
    include_str!("../plugins/lemmaspec/skills/lemmaspec/agents/openai.yaml");

pub const INTRO: &str = r#"LemmaSpec, for an agent

What it is
  A .lemmaspec file is an executable specification: typed relations, facts
  asserted with evidence, rules that derive conclusions, expectations that
  state exact-count claims, and mutation policies that prove the rules are
  load-bearing. The engine evaluates it deterministically: the same file
  always yields the same facts, the same verdict, and the same graph.

Reach for it when
  - a question about code, a plan, or a system can be settled by facts and
    rules rather than by opinion: what is blocked, delivered, untested, at
    risk, or missing;
  - an argument must survive you: another agent or a human should be able to
    audit it, challenge one assumption, and re-run it later;
  - a failed expectation is worth keeping as evidence instead of papering
    over.
  Do not reach for it for mutable runtime state, persistence, or clocks.

The loop
  1. lemmaspec syntax                 the grammar; never guess it
  2. author the artifact              one question per file, from verified
                                      observations only
  3. lemmaspec walk FILE --json       0 clean, 1 an expectation is open,
                                      2 the file itself is wrong
  4. lemmaspec mutate FILE --json     when policies are declared; a survivor
                                      is a specification gap
  5. lemmaspec project FILE --json    the closed graph for other tools
  6. lemmaspec render FILE            the page a human reads

Evidence discipline
  - A fact with provenance is an observation. Without provenance, or below
    100 confidence, it is an assumption: the render lists it as a decision
    waiting to be made and shows what falls if it is wrong.
  - Never promote a guess to a fact to make an expectation pass.
  - An open expectation is a finding. Report it; do not change the count.

Write for the human who will read the render
  - Comments before `spec` state the question. A comment directly above a
    relation, fact, rule, expectation, or mutation explains it.
  - Give relations `roles` and a `reads` template so facts and rule
    conditions render as sentences: "{item} depends on {dependency}".
  - Hand over the HTML with two pointers: press ? for the guide, and the
    Assumptions step is where a decision or evidence is needed.

Install the skill next to the project
  lemmaspec agent install                 .claude/skills and .codex/skills,
                                          matching this binary
  lemmaspec agent install --marketplace   through the agents' own plugin
                                          marketplaces instead
  lemmaspec upgrade --check               is a newer lemmaspec released?
  lemmaspec upgrade                       upgrade the way it was installed,
                                          then re-run agent install"#;

const MARKETPLACE: &str = "MortenHusted/lemmaspec";
const PLUGIN: &str = "lemmaspec@lemmaspec";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Agent {
    Claude,
    Codex,
}

impl Agent {
    pub const ALL: [Agent; 2] = [Agent::Claude, Agent::Codex];

    pub fn name(self) -> &'static str {
        match self {
            Agent::Claude => "Claude Code",
            Agent::Codex => "Codex",
        }
    }

    fn skill_root(self) -> &'static str {
        match self {
            Agent::Claude => ".claude/skills",
            Agent::Codex => ".codex/skills",
        }
    }

    fn cli(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
        }
    }

    /// The commands the README documents for a marketplace install.
    fn marketplace_steps(self) -> [Vec<&'static str>; 2] {
        match self {
            Agent::Claude => [
                vec!["plugin", "marketplace", "add", MARKETPLACE],
                vec!["plugin", "install", PLUGIN],
            ],
            Agent::Codex => [
                vec!["plugin", "marketplace", "add", MARKETPLACE],
                vec!["plugin", "add", PLUGIN],
            ],
        }
    }
}

impl fmt::Display for Agent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Installed {
    pub agent: Agent,
    pub files: Vec<(PathBuf, Change)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Created,
    Updated,
    Unchanged,
}

impl fmt::Display for Change {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Change::Created => "wrote",
            Change::Updated => "updated",
            Change::Unchanged => "unchanged",
        })
    }
}

/// Whether the skill a project carries matches the one in this binary.
pub fn project_skill_is_current(project: &Path) -> Option<bool> {
    let found: Vec<bool> = Agent::ALL
        .iter()
        .map(|agent| {
            project
                .join(agent.skill_root())
                .join("lemmaspec")
                .join("SKILL.md")
        })
        .filter(|path| path.is_file())
        .map(|path| {
            fs::read_to_string(&path)
                .map(|text| text == SKILL)
                .unwrap_or(false)
        })
        .collect();
    if found.is_empty() {
        None
    } else {
        Some(found.iter().all(|current| *current))
    }
}

/// Write the skill this binary was built with into a project's own skill
/// directories. Repeat runs rewrite the same bytes, so the result is stable.
pub fn install_project_skill(project: &Path, agents: &[Agent]) -> Result<Vec<Installed>, String> {
    let mut installed = Vec::new();
    for agent in agents {
        let root = project.join(agent.skill_root()).join("lemmaspec");
        let mut files = vec![write(&root.join("SKILL.md"), SKILL)?];
        if *agent == Agent::Codex {
            files.push(write(
                &root.join("agents").join("openai.yaml"),
                OPENAI_AGENT,
            )?);
        }
        installed.push(Installed {
            agent: *agent,
            files,
        });
    }
    Ok(installed)
}

fn write(path: &Path, content: &str) -> Result<(PathBuf, Change), String> {
    let change = match fs::read_to_string(path) {
        Ok(existing) if existing == content => return Ok((path.to_path_buf(), Change::Unchanged)),
        Ok(_) => Change::Updated,
        Err(_) => Change::Created,
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    fs::write(path, content).map_err(|error| format!("write {}: {error}", path.display()))?;
    Ok((path.to_path_buf(), change))
}

/// Install through the agents' plugin marketplaces by running their CLIs.
/// Returns one report line per agent; a missing CLI is reported, not fatal,
/// unless that agent was the only one asked for.
pub fn install_marketplace(agents: &[Agent]) -> Result<Vec<String>, String> {
    let mut report = Vec::new();
    let mut succeeded = 0;
    for agent in agents {
        let cli = agent.cli();
        let available = Command::new(cli)
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);
        if !available {
            report.push(format!("{agent}: `{cli}` is not on PATH, skipped"));
            continue;
        }
        let mut failed = None;
        for step in agent.marketplace_steps() {
            let status = Command::new(cli)
                .args(&step)
                .status()
                .map_err(|error| format!("run `{cli} {}`: {error}", step.join(" ")))?;
            if !status.success() {
                failed = Some(step.join(" "));
                break;
            }
        }
        match failed {
            Some(step) => report.push(format!("{agent}: `{cli} {step}` failed")),
            None => {
                succeeded += 1;
                report.push(format!("{agent}: plugin {PLUGIN} installed"));
            }
        }
    }
    if succeeded == 0 {
        return Err(format!(
            "no agent CLI completed the marketplace install\n{}",
            report.join("\n")
        ));
    }
    Ok(report)
}
