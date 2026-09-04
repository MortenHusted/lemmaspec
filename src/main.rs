use std::path::{Path, PathBuf};
use std::process::ExitCode;

use lemmaspec::agent::{self, Agent};
use lemmaspec::upgrade::{self, Install};
use lemmaspec::{
    check_artifact, mutate_artifact, project_artifact, render_projection_html, walk_artifact,
    MutationTarget,
};

const HELP: &str = "Usage:
  lemmaspec walk <path.lemmaspec> [--json]
  lemmaspec mutate <path.lemmaspec> [--json]
  lemmaspec check <checker.lemmaspec> <evidence.lemmaspec> [--json]
  lemmaspec project <path.lemmaspec> [--json]
  lemmaspec render <path.lemmaspec> [-o <path.html>]
  lemmaspec syntax
  lemmaspec intro
  lemmaspec agent install [--claude] [--codex] [--dir <project>] [--marketplace]
  lemmaspec upgrade [--check]
  lemmaspec --version
  lemmaspec help [syntax|intro]

Commands:
  walk    Parse, validate, and evaluate one self-contained specification
  mutate  Test whether declared mutations are caught by its expectations
  check   Evaluate a checker's rules over another file's facts and expectations
  project Emit its closed, deterministic graph projection
  render  Write a self-contained human HTML view beside the artifact
  syntax  Show the supported artifact and rule language
  intro   Orient an agent: what LemmaSpec is for and how to work with it
  agent   install: put this binary's skill into a project's .claude and .codex
          skill directories, or into the agents' plugin marketplaces
  upgrade Check GitHub for a newer release and upgrade the way this binary
          was installed (Homebrew, the release installer, or cargo)
  help    Show this help, the syntax reference, or the intro

Options:
  --json  Emit machine-readable JSON for walk, mutate, check, or project
  -o, --output <path.html>  Choose the render output path
  --claude, --codex  Limit agent install to one agent (default: both)
  --dir <project>  Project to install into (default: current directory)
  --marketplace  Run the agents' plugin marketplace commands instead
  --check  Only report whether a newer release exists (exit 1 when it does)
  -V, --version  Show the installed version
  -h, --help

Exit status:
  0  Command succeeded and all expectations passed
  1  Artifact valid, but an expectation or mutation policy failed
  2  Usage, read, parse, validation, or evaluation error

Example:
  lemmaspec walk examples/release_readiness.lemmaspec --json
  lemmaspec mutate examples/mutation_analysis.lemmaspec --json
  lemmaspec check examples/state_as_records.lemmaspec .lemmaspec/state_as_records.lemmaspec
  lemmaspec project examples/release_readiness.lemmaspec --json
  lemmaspec render examples/release_readiness.lemmaspec";

const SYNTAX_HELP: &str = r#"Artifact:
  // Comments before `spec` document the artifact: the question it answers.
  spec NAME {
    // A comment touching a declaration documents that declaration.
    relation NAME {
      args: [symbol, integer]
      roles: [item, score]
      reads: "{item} scored {score}"
    }

    fact ID {
      relation: NAME
      args: [value, 1]
      confidence: 95
      provenance: [source_ref]
    }

    rule ID {
      derive: "result(Item)"
      when: {
        has_input: "input(Item)"
        score_is_sufficient: "Score >= 1"
      }
    }

    expect ID { query: "result(Item)" count: 1 }

    mutation ID {
      operator: drop_condition
      except: [known_equivalent_rule, "rule_id.condition_id"]
      must_fail: EXPECTATION_ID
    }
  }

Artifact rules:
  - relation argument types are symbol or integer
  - roles is an optional identifier per argument; reads is an optional sentence
    template whose {placeholders} name a role or an argument position
  - comments separated from a declaration by a blank line are section headings
    and document nothing; a comment trailing a closing brace documents that block
  - confidence is an optional integer percentage from 0 through 100
  - provenance is an optional list of symbols or strings
  - every predicate used by facts, rules, or expectations needs a relation
  - an asserted relation cannot also be derived by a rule
  - expectations require an exact result count
  - mutation operators are drop_rule, drop_condition, and drop_fact
  - rule when accepts an expression list or a named condition map
  - drop_condition except names rule ids or quoted rule.condition ids
  - drop_fact requires a relation; except names fact ids
  - must_fail names the exact expectation that kills a mutant; omit it for any failure

Logic expressions:
  - variables start with uppercase; `_` is a wildcard
  - symbols may be bare identifiers or quoted strings; integers may be negative
  - bodies support atoms, `!atom(...)`, and <, =<, >, >=, =, \=
  - the right side of a comparison supports integer + and - expressions
  - rule heads support count(X), min(N), and max(N)
  - sum and now are unavailable in self-contained .lemmaspec artifacts

Run `lemmaspec walk FILE --json` to inspect facts and expectations,
`lemmaspec mutate FILE --json` to test the declared mutation policies, then
`lemmaspec project FILE --json` to emit the closed graph."#;

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("lemmaspec: {error}");
            ExitCode::from(2)
        }
    }
}

fn run(args: Vec<String>) -> Result<ExitCode, String> {
    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("lemmaspec {}", env!("CARGO_PKG_VERSION"));
        return Ok(ExitCode::SUCCESS);
    }
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{HELP}");
        return Ok(ExitCode::SUCCESS);
    }
    if args.is_empty() {
        eprintln!("{HELP}");
        return Ok(ExitCode::from(2));
    }
    match args[0].as_str() {
        "walk" => run_walk(&args[1..]),
        "mutate" => run_mutate(&args[1..]),
        "check" => run_check(&args[1..]),
        "project" => run_project(&args[1..]),
        "render" => run_render(&args[1..]),
        "syntax" => print_syntax(&args[1..]),
        "intro" => print_intro(&args[1..]),
        "agent" => run_agent(&args[1..]),
        "upgrade" => run_upgrade(&args[1..]),
        "help" => print_help(&args[1..]),
        command => Err(format!("unknown command `{command}`\n\n{HELP}")),
    }
}

fn print_intro(args: &[String]) -> Result<ExitCode, String> {
    if let Some(unexpected) = args.first() {
        return Err(format!("unexpected argument `{unexpected}` for intro"));
    }
    println!("{}", agent::INTRO);
    Ok(ExitCode::SUCCESS)
}

fn run_agent(args: &[String]) -> Result<ExitCode, String> {
    match args.first().map(String::as_str) {
        Some("install") => {}
        Some(other) => return Err(format!("unknown agent command `{other}`; expected install")),
        None => return Err(format!("agent requires a command: install\n\n{HELP}")),
    }
    let mut agents = Vec::new();
    let mut project = PathBuf::from(".");
    let mut marketplace = false;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--claude" => agents.push(Agent::Claude),
            "--codex" => agents.push(Agent::Codex),
            "--marketplace" => marketplace = true,
            "--dir" => {
                index += 1;
                let Some(dir) = args.get(index) else {
                    return Err("--dir requires a project path".to_string());
                };
                project = PathBuf::from(dir);
            }
            unexpected => {
                return Err(format!(
                    "unexpected argument `{unexpected}` for agent install"
                ))
            }
        }
        index += 1;
    }
    if agents.is_empty() {
        agents.extend(Agent::ALL);
    }
    if marketplace {
        for line in agent::install_marketplace(&agents)? {
            println!("{line}");
        }
        return Ok(ExitCode::SUCCESS);
    }
    if !project.is_dir() {
        return Err(format!("`{}` is not a directory", project.display()));
    }
    for installed in agent::install_project_skill(&project, &agents)? {
        for (file, change) in installed.files {
            println!("{}: {change} {}", installed.agent, file.display());
        }
    }
    println!(
        "skill version matches lemmaspec {}",
        env!("CARGO_PKG_VERSION")
    );
    Ok(ExitCode::SUCCESS)
}

fn run_upgrade(args: &[String]) -> Result<ExitCode, String> {
    let check_only = match args {
        [] => false,
        [flag] if flag == "--check" => true,
        [unexpected, ..] => return Err(format!("unexpected argument `{unexpected}` for upgrade")),
    };
    let current = env!("CARGO_PKG_VERSION");
    let install = Install::detect();
    println!("installed: lemmaspec {current} via {install}");
    let latest = upgrade::latest_version()?;
    println!("latest:    lemmaspec {latest}");
    let behind = upgrade::compare_versions(current, &latest) == std::cmp::Ordering::Less;
    if let Some(false) = agent::project_skill_is_current(Path::new(".")) {
        println!("note: the skill in this project differs from this binary; run `lemmaspec agent install`");
    }
    if !behind {
        println!("up to date");
        return Ok(ExitCode::SUCCESS);
    }
    if check_only {
        println!("newer release available");
        return Ok(ExitCode::from(1));
    }
    let Some(command) = install.upgrade_command() else {
        println!("{}", install.advice());
        return Ok(ExitCode::from(1));
    };
    println!("running: {}", command.join(" "));
    let status = std::process::Command::new(&command[0])
        .args(&command[1..])
        .status()
        .map_err(|error| format!("run `{}`: {error}", command.join(" ")))?;
    if !status.success() {
        return Err(format!("`{}` failed", command.join(" ")));
    }
    println!("upgraded to lemmaspec {latest}; run `lemmaspec agent install` in each project to refresh its skill");
    Ok(ExitCode::SUCCESS)
}

fn print_help(args: &[String]) -> Result<ExitCode, String> {
    match args {
        [] => {
            println!("{HELP}");
            Ok(ExitCode::SUCCESS)
        }
        [topic] if topic == "syntax" => print_syntax(&[]),
        [topic] if topic == "intro" => print_intro(&[]),
        [topic] => Err(format!("unknown help topic `{topic}`\n\n{HELP}")),
        _ => Err(format!("help accepts at most one topic\n\n{HELP}")),
    }
}

fn print_syntax(args: &[String]) -> Result<ExitCode, String> {
    if let Some(unexpected) = args.first() {
        return Err(format!("unexpected argument `{unexpected}` for syntax"));
    }
    println!("{SYNTAX_HELP}");
    Ok(ExitCode::SUCCESS)
}

fn artifact_path<'a>(args: &'a [String], command: &str) -> Result<&'a str, String> {
    let Some(path) = args.first() else {
        return Err(format!("{command} requires a .lemmaspec path\n\n{HELP}"));
    };
    if let Some(unexpected) = args[1..].iter().find(|arg| arg.as_str() != "--json") {
        return Err(format!("unexpected argument `{unexpected}`"));
    }
    if Path::new(path).extension().and_then(|value| value.to_str()) != Some("lemmaspec") {
        return Err(format!("{command} expects a .lemmaspec file, got `{path}`"));
    }
    Ok(path)
}

fn run_walk(args: &[String]) -> Result<ExitCode, String> {
    let path = artifact_path(args, "walk")?;

    let source =
        std::fs::read_to_string(path).map_err(|error| format!("read `{path}`: {error}"))?;
    let report = walk_artifact(&source).map_err(|error| format!("walk `{path}`: {error}"))?;
    report_walk(&report, args.iter().any(|arg| arg == "--json"))
}

fn run_check(args: &[String]) -> Result<ExitCode, String> {
    let (paths, flags): (Vec<&String>, Vec<&String>) =
        args.iter().partition(|arg| !arg.starts_with("--"));
    let [checker, evidence] = paths[..] else {
        return Err(format!(
            "check requires a checker and an evidence .lemmaspec path\n\n{HELP}"
        ));
    };
    if let Some(unexpected) = flags.iter().find(|flag| flag.as_str() != "--json") {
        return Err(format!("unexpected argument `{unexpected}`"));
    }
    for path in [checker, evidence] {
        if Path::new(path).extension().and_then(|value| value.to_str()) != Some("lemmaspec") {
            return Err(format!("check expects .lemmaspec files, got `{path}`"));
        }
    }

    let read = |path: &String| {
        std::fs::read_to_string(path).map_err(|error| format!("read `{path}`: {error}"))
    };
    let report = check_artifact(&read(checker)?, &read(evidence)?)
        .map_err(|error| format!("check `{checker}` over `{evidence}`: {error}"))?;
    report_walk(&report, !flags.is_empty())
}

fn report_walk(report: &lemmaspec::WalkReport, json: bool) -> Result<ExitCode, String> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(report)
                .map_err(|error| format!("serialize walk report: {error}"))?
        );
    } else {
        println!(
            "{}: {} ({} asserted, {} derived)",
            report.spec, report.status, report.asserted, report.derived
        );
        for expectation in &report.expectations {
            let mark = if expectation.satisfied { "ok" } else { "FAIL" };
            println!(
                "  [{mark}] {}: {} (expected {}, found {})",
                expectation.id,
                expectation.query,
                expectation.expected_count,
                expectation.actual_count
            );
            if !expectation.satisfied {
                for fact in &expectation.found {
                    println!("         {fact}");
                }
            }
        }
    }

    Ok(if report.status == "clean" {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn run_mutate(args: &[String]) -> Result<ExitCode, String> {
    let path = artifact_path(args, "mutate")?;
    let source =
        std::fs::read_to_string(path).map_err(|error| format!("read `{path}`: {error}"))?;
    let report = mutate_artifact(&source).map_err(|error| format!("mutate `{path}`: {error}"))?;

    if args.iter().any(|arg| arg == "--json") {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|error| format!("serialize mutation report: {error}"))?
        );
    } else {
        println!(
            "{}: {} ({} killed, {} survived, {} rejected, {} excluded)",
            report.spec,
            report.status,
            report.summary.killed,
            report.summary.survived,
            report.summary.rejected,
            report.summary.excluded
        );
        for failure in &report.baseline_failures {
            println!(
                "  [baseline FAIL] {}: {} (expected {}, found {})",
                failure.id, failure.query, failure.expected_count, failure.actual_count
            );
        }
        for policy in &report.policies {
            println!(
                "  [policy {:<8}] {} ({} executed, {} rejected, {} excluded)",
                policy.status,
                policy.id,
                policy.summary.executed,
                policy.summary.rejected,
                policy.summary.excluded
            );
        }
        for mutation in &report.mutations {
            let target = match &mutation.target {
                MutationTarget::Rule { rule } => format!("rule {rule}"),
                MutationTarget::Condition {
                    rule,
                    condition,
                    index,
                    expression,
                } => condition.as_deref().map_or_else(
                    || format!("condition {index} of rule {rule}: {expression}"),
                    |condition| format!("condition {condition} of rule {rule}: {expression}"),
                ),
                MutationTarget::Fact { fact, relation } => {
                    format!("fact {fact} of relation {relation}")
                }
            };
            let failures = mutation
                .failed_expectations
                .iter()
                .map(|expectation| expectation.id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let detail = if let Some(diagnostic) = &mutation.diagnostic {
                format!(" ({diagnostic})")
            } else if failures.is_empty() {
                String::new()
            } else {
                format!(" (failed: {failures})")
            };
            println!(
                "  [{:<8}] {} -> {}{}",
                mutation.status.as_str(),
                mutation.policy,
                target,
                detail
            );
        }
    }

    Ok(if report.status == "clean" {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn run_project(args: &[String]) -> Result<ExitCode, String> {
    let path = artifact_path(args, "project")?;
    let source =
        std::fs::read_to_string(path).map_err(|error| format!("read `{path}`: {error}"))?;
    let projection =
        project_artifact(&source).map_err(|error| format!("project `{path}`: {error}"))?;

    if args.iter().any(|arg| arg == "--json") {
        println!(
            "{}",
            serde_json::to_string_pretty(&projection)
                .map_err(|error| format!("serialize graph projection: {error}"))?
        );
    } else {
        println!(
            "{}: {} ({} nodes, {} edges)",
            projection.spec,
            projection.status,
            projection.nodes.len(),
            projection.edges.len()
        );
    }

    Ok(if projection.status == "clean" {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn run_render(args: &[String]) -> Result<ExitCode, String> {
    let (path, output_path) = render_paths(args)?;
    let source =
        std::fs::read_to_string(path).map_err(|error| format!("read `{path}`: {error}"))?;
    let projection =
        project_artifact(&source).map_err(|error| format!("render `{path}`: {error}"))?;
    let html = render_projection_html(&source, &projection);

    if let Some(parent) = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create output directory `{}`: {error}", parent.display()))?;
    }
    std::fs::write(&output_path, html)
        .map_err(|error| format!("write `{}`: {error}", output_path.display()))?;
    println!(
        "wrote {} ({}; {} nodes, {} edges)",
        output_path.display(),
        projection.status,
        projection.nodes.len(),
        projection.edges.len()
    );

    Ok(if projection.status == "clean" {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn render_paths(args: &[String]) -> Result<(&str, PathBuf), String> {
    let Some(path) = args.first() else {
        return Err(format!("render requires a .lemmaspec path\n\n{HELP}"));
    };
    if Path::new(path).extension().and_then(|value| value.to_str()) != Some("lemmaspec") {
        return Err(format!("render expects a .lemmaspec file, got `{path}`"));
    }

    let mut output = None;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "-o" | "--output" => {
                if output.is_some() {
                    return Err("render accepts only one output path".to_string());
                }
                let Some(value) = args.get(index + 1) else {
                    return Err(format!("{} requires a path", args[index]));
                };
                output = Some(PathBuf::from(value));
                index += 2;
            }
            unexpected => return Err(format!("unexpected argument `{unexpected}`")),
        }
    }
    let output = output.unwrap_or_else(|| Path::new(path).with_extension("html"));
    if output.extension().and_then(|value| value.to_str()) != Some("html") {
        return Err(format!(
            "render output must be an .html file, got `{}`",
            output.display()
        ));
    }
    Ok((path, output))
}
