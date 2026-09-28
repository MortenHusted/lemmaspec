use std::{fs, process::Command};

#[test]
fn cli_projects_to_file_and_rejects_invalid_options_without_writing() {
    let output = std::env::temp_dir().join(format!(
        "lemmaspec-plan-cli-{}.lemmaspec",
        std::process::id()
    ));
    let plan = format!("{}/tests/fixtures/plan.md", env!("CARGO_MANIFEST_DIR"));
    let run = Command::new(env!("CARGO_BIN_EXE_lemmaspec-plan"))
        .args(["project", &plan, "-o"])
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let projected = fs::read_to_string(&output).unwrap();
    assert_eq!(
        lemmaspec::check_artifact(lemmaspec_plan::CHECKER, &projected)
            .unwrap()
            .status,
        "clean"
    );
    let invalid = Command::new(env!("CARGO_BIN_EXE_lemmaspec-plan"))
        .args(["project", &plan, "-o"])
        .arg(&output)
        .arg("--unexpected")
        .output()
        .unwrap();
    assert!(!invalid.status.success());
    assert_eq!(fs::read_to_string(&output).unwrap(), projected);
    fs::remove_file(output).unwrap();
    let checker = Command::new(env!("CARGO_BIN_EXE_lemmaspec-plan"))
        .arg("checker")
        .output()
        .unwrap();
    assert!(checker.status.success());
    assert_eq!(
        String::from_utf8(checker.stdout).unwrap(),
        lemmaspec_plan::CHECKER
    );
}
