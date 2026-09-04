use std::collections::BTreeSet;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn version_flags_report_the_package_version_globally() {
    for args in [
        &["--version"][..],
        &["-V"],
        &["walk", "--version"],
        &["syntax", "-V"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_lemmaspec"))
            .args(args)
            .output()
            .expect("run lemmaspec");

        assert!(output.status.success(), "{args:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            format!("lemmaspec {}\n", env!("CARGO_PKG_VERSION")),
            "{args:?}"
        );
        assert!(output.stderr.is_empty(), "{args:?}");
    }
}

#[test]
fn walk_command_emits_machine_readable_report() {
    let output = Command::new(env!("CARGO_BIN_EXE_lemmaspec"))
        .args(["walk", "examples/release_readiness.lemmaspec", "--json"])
        .output()
        .expect("run lemmaspec");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON report");
    assert_eq!(report["spec"], "release_readiness");
    assert_eq!(report["status"], "clean");
    assert_eq!(report["asserted"], 2);
    assert_eq!(report["derived"], 1);
}

#[test]
fn mutate_command_emits_machine_readable_report() {
    let output = Command::new(env!("CARGO_BIN_EXE_lemmaspec"))
        .args(["mutate", "examples/mutation_analysis.lemmaspec", "--json"])
        .output()
        .expect("run lemmaspec");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON report");
    assert_eq!(report["spec"], "mutation_analysis");
    assert_eq!(report["status"], "clean");
    assert_eq!(report["summary"]["total"], 9);
    assert_eq!(report["summary"]["executed"], 9);
    assert_eq!(report["summary"]["killed"], 9);
    assert_eq!(report["summary"]["survived"], 0);
    assert_eq!(report["policies"][0]["status"], "clean");
}

#[test]
fn mutate_command_prints_named_condition_targets() {
    let source = r#"
spec named_condition_cli {
  relation source { args: [symbol] }
  relation allowed { args: [symbol] }
  relation result { args: [symbol] }
  fact source_release { relation: source args: [release] }
  fact source_draft { relation: source args: [draft] }
  fact allowed_release { relation: allowed args: [release] }
  rule derive_result {
    derive: "result(Item)"
    when: {
      source_exists: "source(Item)"
      item_is_allowed: "allowed(Item)"
    }
  }
  expect exactly_one_result { query: "result(Item)" count: 1 }
  mutation condition_coverage {
    operator: drop_condition
    except: ["derive_result.source_exists"]
    must_fail: exactly_one_result
  }
}
"#;
    let directory = temporary_directory("named-condition-output");
    let input = directory.join("named.lemmaspec");
    std::fs::write(&input, source).expect("write artifact");

    let output = Command::new(env!("CARGO_BIN_EXE_lemmaspec"))
        .args(["mutate", input.to_str().unwrap()])
        .output()
        .expect("run lemmaspec");
    std::fs::remove_dir_all(directory).expect("remove temporary directory");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("condition source_exists of rule derive_result: source(Item)"),
        "{stdout}"
    );
    assert!(
        stdout.contains("condition item_is_allowed of rule derive_result: allowed(Item)"),
        "{stdout}"
    );
}

#[test]
fn surviving_mutant_exits_one() {
    let source = r#"
spec surviving_mutant {
  relation pair { args: [symbol, symbol] }
  relation related { args: [symbol, symbol] }
  fact pair_ab { relation: pair args: [a, b] }
  rule relate {
    derive: "related(A, B)"
    when: ["pair(A, B)", "A \\= B"]
  }
  expect one_pair { query: "related(A, B)" count: 1 }
  mutation condition_coverage { operator: drop_condition }
}
"#;
    let directory = temporary_directory("mutation-survivor");
    let input = directory.join("survivor.lemmaspec");
    std::fs::write(&input, source).expect("write artifact");

    let output = Command::new(env!("CARGO_BIN_EXE_lemmaspec"))
        .args(["mutate", input.to_str().unwrap(), "--json"])
        .output()
        .expect("run lemmaspec");
    std::fs::remove_dir_all(directory).expect("remove temporary directory");

    assert_eq!(output.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON report");
    assert_eq!(report["status"], "survived");
    assert_eq!(report["summary"]["survived"], 1);
    assert_eq!(report["summary"]["rejected"], 1);
}

#[test]
fn vacuous_mutation_policy_exits_one() {
    let source = r#"
spec vacuous_mutation {
  relation input { args: [symbol] }
  relation output { args: [symbol] }
  fact input_exists { relation: input args: [value] }
  rule derive_output {
    derive: "output(Value)"
    when: ["input(Value)"]
  }
  expect output_exists { query: "output(value)" count: 1 }
  mutation condition_coverage { operator: drop_condition }
}
"#;
    let directory = temporary_directory("mutation-vacuity");
    let input = directory.join("vacuous.lemmaspec");
    std::fs::write(&input, source).expect("write artifact");

    let output = Command::new(env!("CARGO_BIN_EXE_lemmaspec"))
        .args(["mutate", input.to_str().unwrap(), "--json"])
        .output()
        .expect("run lemmaspec");
    std::fs::remove_dir_all(directory).expect("remove temporary directory");

    assert_eq!(output.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON report");
    assert_eq!(report["status"], "vacuous");
    assert_eq!(report["summary"]["executed"], 0);
    assert_eq!(report["policies"][0]["status"], "vacuous");
}

#[test]
fn project_command_emits_a_closed_native_graph_shape() {
    let output = Command::new(env!("CARGO_BIN_EXE_lemmaspec"))
        .args(["project", "examples/release_readiness.lemmaspec", "--json"])
        .output()
        .expect("run lemmaspec");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let graph: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON graph");
    let nodes = graph["nodes"].as_array().expect("nodes");
    let edges = graph["edges"].as_array().expect("edges");
    let node_ids: BTreeSet<_> = nodes
        .iter()
        .filter_map(|node| node["id"].as_str())
        .collect();

    assert!(nodes.iter().all(|node| node["kind"] == "node"));
    assert!(edges.iter().all(|edge| {
        edge["kind"] == "edge"
            && edge["from"]
                .as_str()
                .is_some_and(|id| node_ids.contains(id))
            && edge["to"].as_str().is_some_and(|id| node_ids.contains(id))
    }));
}

#[test]
fn help_describes_machine_output_and_exit_statuses() {
    let output = Command::new(env!("CARGO_BIN_EXE_lemmaspec"))
        .arg("--help")
        .output()
        .expect("run lemmaspec");

    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(help.contains("syntax"), "{help}");
    assert!(
        help.contains("machine-readable JSON for walk, mutate, or project"),
        "{help}"
    );
    assert!(
        help.contains("1  Artifact valid, but an expectation or mutation policy failed"),
        "{help}"
    );
    assert!(
        help.contains("2  Usage, read, parse, validation, or evaluation error"),
        "{help}"
    );
}

#[test]
fn syntax_command_prints_the_supported_artifact_language() {
    let output = Command::new(env!("CARGO_BIN_EXE_lemmaspec"))
        .arg("syntax")
        .output()
        .expect("run lemmaspec");

    assert!(output.status.success());
    let syntax = String::from_utf8_lossy(&output.stdout);
    assert!(syntax.contains("spec NAME"), "{syntax}");
    assert!(syntax.contains("relation NAME"), "{syntax}");
    assert!(syntax.contains("mutation ID"), "{syntax}");
    assert!(syntax.contains("drop_condition"), "{syntax}");
    assert!(syntax.contains("named condition map"), "{syntax}");
    assert!(syntax.contains("quoted rule.condition ids"), "{syntax}");
    assert!(syntax.contains("count(X)"), "{syntax}");
    assert!(syntax.contains("sum and now are unavailable"), "{syntax}");
    assert!(
        syntax
            .lines()
            .any(|line| line.ends_with("and <, =<, >, >=, =, \\=")),
        "{syntax}"
    );
}

#[test]
fn render_command_writes_html_beside_the_artifact_by_default() {
    let directory = temporary_directory("render-clean");
    let input = directory.join("release.lemmaspec");
    let output = directory.join("release.html");
    std::fs::write(
        &input,
        include_str!("../examples/release_readiness.lemmaspec"),
    )
    .expect("write artifact");

    let result = Command::new(env!("CARGO_BIN_EXE_lemmaspec"))
        .args(["render", input.to_str().unwrap()])
        .output()
        .expect("run lemmaspec");

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output.is_file());
    assert!(String::from_utf8_lossy(&result.stdout).contains(output.to_str().unwrap()));
    let html = std::fs::read_to_string(output).expect("read HTML");
    assert!(
        html.starts_with("<!doctype html>\n<html lang=\"en\" data-status=\"clean\">"),
        "{html}"
    );
    std::fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn incomplete_render_writes_html_and_exits_one() {
    let directory = temporary_directory("render-incomplete");
    let input = directory.join("release.lemmaspec");
    let output = directory.join("report.html");
    let source =
        include_str!("../examples/release_readiness.lemmaspec").replace("count: 1", "count: 0");
    std::fs::write(&input, source).expect("write artifact");

    let result = Command::new(env!("CARGO_BIN_EXE_lemmaspec"))
        .args([
            "render",
            input.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("run lemmaspec");

    assert_eq!(result.status.code(), Some(1));
    let html = std::fs::read_to_string(&output).expect("read HTML");
    assert!(
        html.starts_with("<!doctype html>\n<html lang=\"en\" data-status=\"incomplete\">"),
        "{html}"
    );
    assert!(html.contains("1 expectation needs attention"), "{html}");
    assert!(html.contains("data-status=\"failed\""), "{html}");
    std::fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn invalid_render_does_not_write_html() {
    let directory = temporary_directory("render-invalid");
    let input = directory.join("invalid.lemmaspec");
    let output = directory.join("invalid.html");
    std::fs::write(&input, "spec invalid { fact }").expect("write invalid artifact");

    let result = Command::new(env!("CARGO_BIN_EXE_lemmaspec"))
        .args(["render", input.to_str().unwrap()])
        .output()
        .expect("run lemmaspec");

    assert_eq!(result.status.code(), Some(2));
    assert!(!output.exists());
    let stderr = String::from_utf8_lossy(&result.stderr);
    let prefix = format!("lemmaspec: render `{}`: ", input.display());
    let diagnostic = stderr
        .strip_prefix(&prefix)
        .expect("render-specific diagnostic prefix");
    assert!(diagnostic.contains("expected block name"), "{diagnostic}");
    std::fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn render_rejects_invalid_cli_shapes_without_writing_output() {
    let directory = temporary_directory("render-cli-errors");
    let input = directory.join("release.lemmaspec");
    let wrong_extension = directory.join("release.txt");
    std::fs::write(
        &input,
        include_str!("../examples/release_readiness.lemmaspec"),
    )
    .expect("write artifact");
    std::fs::write(&wrong_extension, "not a LemmaSpec artifact").expect("write text input");

    let default_output = directory.join("release.html");
    let first_output = directory.join("first.html");
    let second_output = directory.join("second.html");
    let non_html_output = directory.join("report.txt");
    let cases = [
        (
            "missing input",
            vec!["render".to_string()],
            "render requires a .lemmaspec path",
            vec![],
        ),
        (
            "non-.lemmaspec input",
            vec!["render".to_string(), wrong_extension.display().to_string()],
            "render expects a .lemmaspec file",
            vec![default_output.clone()],
        ),
        (
            "missing output value",
            vec![
                "render".to_string(),
                input.display().to_string(),
                "--output".to_string(),
            ],
            "--output requires a path",
            vec![default_output.clone()],
        ),
        (
            "duplicate output",
            vec![
                "render".to_string(),
                input.display().to_string(),
                "--output".to_string(),
                first_output.display().to_string(),
                "-o".to_string(),
                second_output.display().to_string(),
            ],
            "render accepts only one output path",
            vec![first_output.clone(), second_output.clone()],
        ),
        (
            "unexpected argument",
            vec![
                "render".to_string(),
                input.display().to_string(),
                "unexpected".to_string(),
            ],
            "unexpected argument `unexpected`",
            vec![default_output.clone()],
        ),
        (
            "non-.html output",
            vec![
                "render".to_string(),
                input.display().to_string(),
                "--output".to_string(),
                non_html_output.display().to_string(),
            ],
            "render output must be an .html file",
            vec![non_html_output.clone()],
        ),
    ];

    for (label, args, expected_error, output_paths) in cases {
        let result = Command::new(env!("CARGO_BIN_EXE_lemmaspec"))
            .args(args)
            .output()
            .expect("run lemmaspec");

        assert_eq!(result.status.code(), Some(2), "{label}");
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(
            stderr.starts_with("lemmaspec: ") && stderr.contains(expected_error),
            "{label}: {stderr}"
        );
        for output_path in output_paths {
            assert!(!output_path.exists(), "{label}: {}", output_path.display());
        }
    }

    std::fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn render_output_is_byte_stable_across_processes() {
    let directory = temporary_directory("render-stable");
    let input = directory.join("release.lemmaspec");
    let first = directory.join("first.html");
    let second = directory.join("second.html");
    std::fs::write(
        &input,
        include_str!("../examples/release_readiness.lemmaspec"),
    )
    .expect("write artifact");

    for output in [&first, &second] {
        let result = Command::new(env!("CARGO_BIN_EXE_lemmaspec"))
            .args([
                "render",
                input.to_str().unwrap(),
                "--output",
                output.to_str().unwrap(),
            ])
            .output()
            .expect("run lemmaspec");
        assert!(result.status.success());
    }

    assert_eq!(
        std::fs::read(&first).expect("read first HTML"),
        std::fs::read(&second).expect("read second HTML")
    );
    std::fs::remove_dir_all(directory).expect("remove temporary directory");
}

#[test]
fn bare_invocation_prints_help_and_fails() {
    let output = Command::new(env!("CARGO_BIN_EXE_lemmaspec"))
        .output()
        .expect("run lemmaspec");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Usage:"));
}

#[test]
fn incomplete_walk_exits_one_and_reports_failure() {
    let source =
        include_str!("../examples/release_readiness.lemmaspec").replace("count: 1", "count: 0");
    let path = std::env::temp_dir().join(format!(
        "lemmaspec-incomplete-{}.lemmaspec",
        std::process::id()
    ));
    std::fs::write(&path, source).expect("write temporary artifact");

    let output = Command::new(env!("CARGO_BIN_EXE_lemmaspec"))
        .args(["walk", path.to_str().unwrap(), "--json"])
        .output()
        .expect("run lemmaspec");
    std::fs::remove_file(&path).expect("remove temporary artifact");

    assert_eq!(output.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON report");
    assert_eq!(report["status"], "incomplete");
    assert_eq!(report["expectations"][0]["satisfied"], false);
}

#[test]
fn incomplete_project_exits_one_and_omits_proof_edges() {
    let source =
        include_str!("../examples/release_readiness.lemmaspec").replace("count: 1", "count: 0");
    let path = std::env::temp_dir().join(format!(
        "lemmaspec-incomplete-project-{}.lemmaspec",
        std::process::id()
    ));
    std::fs::write(&path, source).expect("write temporary artifact");

    let output = Command::new(env!("CARGO_BIN_EXE_lemmaspec"))
        .args(["project", path.to_str().unwrap(), "--json"])
        .output()
        .expect("run lemmaspec");
    std::fs::remove_file(&path).expect("remove temporary artifact");

    assert_eq!(output.status.code(), Some(1));
    let graph: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON graph");
    assert_eq!(graph["status"], "incomplete");
    assert!(graph["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .any(|node| { node["type"] == "expectation" && node["satisfied"] == false }));
    assert!(graph["edges"]
        .as_array()
        .expect("edges")
        .iter()
        .all(|edge| edge["rel"] != "proves"));
}

#[test]
fn missing_artifact_exits_two() {
    let output = Command::new(env!("CARGO_BIN_EXE_lemmaspec"))
        .args(["walk", "does-not-exist.lemmaspec"])
        .output()
        .expect("run lemmaspec");

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("read `does-not-exist.lemmaspec`"));
}

#[test]
fn intro_orients_an_agent() {
    let output = Command::new(env!("CARGO_BIN_EXE_lemmaspec"))
        .arg("intro")
        .output()
        .expect("run intro");
    assert_eq!(output.status.code(), Some(0));
    let intro = String::from_utf8_lossy(&output.stdout);
    for expected in [
        "lemmaspec walk FILE --json",
        "lemmaspec mutate FILE --json",
        "lemmaspec render FILE",
        "assumption",
        "Never promote a guess to a fact",
        "lemmaspec agent install",
    ] {
        assert!(
            intro.contains(expected),
            "intro lacks {expected:?}:\n{intro}"
        );
    }

    let via_help = Command::new(env!("CARGO_BIN_EXE_lemmaspec"))
        .args(["help", "intro"])
        .output()
        .expect("run help intro");
    assert_eq!(via_help.stdout, output.stdout);
}

#[test]
fn agent_install_writes_this_binarys_skill_into_a_project() {
    let project = temporary_directory("agent-install");
    let output = Command::new(env!("CARGO_BIN_EXE_lemmaspec"))
        .args(["agent", "install", "--dir", project.to_str().unwrap()])
        .output()
        .expect("run agent install");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let claude = project.join(".claude/skills/lemmaspec/SKILL.md");
    let codex = project.join(".codex/skills/lemmaspec/SKILL.md");
    let openai = project.join(".codex/skills/lemmaspec/agents/openai.yaml");
    let skill = include_str!("../plugins/lemmaspec/skills/lemmaspec/SKILL.md");
    assert_eq!(
        std::fs::read_to_string(&claude).expect("claude skill"),
        skill
    );
    assert_eq!(std::fs::read_to_string(&codex).expect("codex skill"), skill);
    assert_eq!(
        std::fs::read_to_string(&openai).expect("codex agent metadata"),
        include_str!("../plugins/lemmaspec/skills/lemmaspec/agents/openai.yaml")
    );
    assert!(!project.join(".claude/skills/lemmaspec/agents").exists());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Claude Code: wrote"), "{stdout}");
    assert!(stdout.contains("Codex: wrote"), "{stdout}");
    assert!(
        stdout.contains("skill version matches lemmaspec"),
        "{stdout}"
    );

    let again = Command::new(env!("CARGO_BIN_EXE_lemmaspec"))
        .args(["agent", "install", "--dir", project.to_str().unwrap()])
        .output()
        .expect("run agent install again");
    let again = String::from_utf8_lossy(&again.stdout);
    assert!(again.contains("Claude Code: unchanged"), "{again}");
    assert!(!again.contains("wrote"), "{again}");

    std::fs::write(&codex, "stale").expect("age the codex skill");
    let refreshed = Command::new(env!("CARGO_BIN_EXE_lemmaspec"))
        .args(["agent", "install", "--dir", project.to_str().unwrap()])
        .output()
        .expect("run agent install over a stale skill");
    let refreshed = String::from_utf8_lossy(&refreshed.stdout);
    assert!(refreshed.contains("Codex: updated"), "{refreshed}");
    assert_eq!(std::fs::read_to_string(&codex).expect("codex skill"), skill);

    let only_codex = temporary_directory("agent-install-codex");
    let output = Command::new(env!("CARGO_BIN_EXE_lemmaspec"))
        .args([
            "agent",
            "install",
            "--codex",
            "--dir",
            only_codex.to_str().unwrap(),
        ])
        .output()
        .expect("run codex-only install");
    assert_eq!(output.status.code(), Some(0));
    assert!(only_codex.join(".codex/skills/lemmaspec/SKILL.md").exists());
    assert!(!only_codex.join(".claude").exists());

    let bad = Command::new(env!("CARGO_BIN_EXE_lemmaspec"))
        .args([
            "agent",
            "install",
            "--dir",
            project.join("missing").to_str().unwrap(),
        ])
        .output()
        .expect("run install into a missing directory");
    assert_eq!(bad.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&bad.stderr).contains("is not a directory"));

    std::fs::remove_dir_all(&project).ok();
    std::fs::remove_dir_all(&only_codex).ok();
}

#[test]
fn upgrade_rejects_unknown_arguments_and_is_listed_in_help() {
    let output = Command::new(env!("CARGO_BIN_EXE_lemmaspec"))
        .args(["upgrade", "--now"])
        .output()
        .expect("run upgrade with a bad flag");
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("unexpected argument `--now`"));

    let help = Command::new(env!("CARGO_BIN_EXE_lemmaspec"))
        .arg("--help")
        .output()
        .expect("run help");
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(help.contains("lemmaspec upgrade [--check]"), "{help}");
    assert!(help.contains("lemmaspec agent install"), "{help}");
    assert!(help.contains("lemmaspec intro"), "{help}");
}

fn temporary_directory(label: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time after epoch")
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("lemmaspec-{label}-{}-{nonce}", std::process::id()));
    std::fs::create_dir_all(&path).expect("create temporary directory");
    path
}
