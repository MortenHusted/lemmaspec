use std::{fs, path::PathBuf, process::Command};

use lemmaspec_intent::identity::Repo;

/// A fresh git repository under the system temp directory, removed on drop.
struct TempRepo {
    dir: PathBuf,
}

impl TempRepo {
    fn new(label: &str) -> TempRepo {
        let dir = std::env::temp_dir().join(format!(
            "lemmaspec-intent-identity-{label}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let repo = TempRepo { dir };
        repo.git(&["init", "-q"]);
        for (key, value) in [
            ("user.email", "intent@example.com"),
            ("user.name", "Intent Test"),
            ("commit.gpgsign", "false"),
            ("core.autocrlf", "false"),
            ("core.hooksPath", "no-hooks"),
        ] {
            repo.git(&["config", key, value]);
        }
        repo
    }

    /// A small project whose catalog lives in `intent/`.
    fn with_project(label: &str) -> TempRepo {
        let repo = TempRepo::new(label);
        repo.write("src/lib.rs", "pub fn answer() -> u8 {\n    42\n}\n");
        repo.write("README.md", "# Fixture\n");
        repo.write("intent/catalog.intent", "catalog fixture {\n}\n");
        repo.write("intent/runs.intent", "// runs\n");
        repo.write("intent/approvals.intent", "// approvals\n");
        repo
    }

    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.dir)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }

    fn write(&self, path: &str, content: &str) {
        let path = self.dir.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn read(&self, path: &str) -> String {
        fs::read_to_string(self.dir.join(path)).unwrap()
    }

    fn commit(&self, message: &str) -> String {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "--no-verify", "-m", message]);
        self.git(&["rev-parse", "HEAD"]).trim().to_string()
    }

    fn source(&self) -> Repo {
        Repo::of_catalog(&self.dir.join("intent")).unwrap()
    }

    fn identity(&self) -> String {
        self.source().working_tree_identity().unwrap()
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn same_tracked_content_gives_the_same_identity_and_any_tracked_edit_changes_it() {
    let first = TempRepo::with_project("same-first");
    first.commit("first");
    let second = TempRepo::with_project("same-second");
    second.commit("a different commit of the same content");

    let identity = first.identity();
    assert!(identity.starts_with("sha256:"), "{identity}");
    assert_eq!(first.identity(), identity);
    assert_eq!(second.identity(), identity);

    for (path, edited) in [
        ("src/lib.rs", "pub fn answer() -> u8 {\n    41\n}\n"),
        ("README.md", "# Fixture, edited\n"),
        (
            "intent/catalog.intent",
            "catalog fixture {\n  // edited\n}\n",
        ),
    ] {
        let original = first.read(path);
        first.write(path, edited);
        assert_ne!(first.identity(), identity, "editing {path}");
        first.write(path, &original);
        assert_eq!(first.identity(), identity, "restoring {path}");
    }

    first.write("notes.txt", "draft\n");
    assert_eq!(
        first.identity(),
        identity,
        "an untracked file is not source"
    );
    first.git(&["add", "notes.txt"]);
    assert_ne!(first.identity(), identity, "a staged new file is tracked");
}

#[test]
fn a_clean_checkout_has_the_identity_of_its_commit_tree() {
    let repo = TempRepo::with_project("checkout");
    let first = repo.commit("first");
    repo.write("src/lib.rs", "pub fn answer() -> u8 {\n    43\n}\n");
    let second = repo.commit("second");
    let source = repo.source();

    assert_eq!(source.head().unwrap(), second);
    assert_eq!(
        source.working_tree_identity().unwrap(),
        source.commit_identity(&second).unwrap()
    );
    assert!(source.matches_head().unwrap());
    assert_ne!(
        source.commit_identity(&first).unwrap(),
        source.commit_identity(&second).unwrap()
    );

    repo.git(&["checkout", "-q", &first]);
    assert_eq!(source.head().unwrap(), first);
    assert_eq!(
        source.working_tree_identity().unwrap(),
        source.commit_identity(&first).unwrap()
    );
    assert!(source.matches_head().unwrap());

    repo.write("src/lib.rs", "pub fn answer() -> u8 {\n    0\n}\n");
    assert!(
        !source.matches_head().unwrap(),
        "a scratch break is not HEAD"
    );
}

#[test]
fn a_crlf_working_tree_under_eol_lf_has_the_identity_of_its_lf_commit() {
    let repo = TempRepo::with_project("crlf");
    repo.write(".gitattributes", "* text eol=lf\n*.bin -text\n");
    repo.write("notes.txt", "one\ntwo\n");
    repo.write("data.bin", "one\ntwo\n");
    let head = repo.commit("lf");
    let source = repo.source();
    let committed = source.commit_identity(&head).unwrap();

    repo.write("notes.txt", "one\r\ntwo\r\n");
    assert_eq!(source.working_tree_identity().unwrap(), committed);
    assert!(source.matches_head().unwrap());

    repo.write("data.bin", "one\r\ntwo\r\n");
    assert_ne!(
        source.working_tree_identity().unwrap(),
        committed,
        "a file outside the text rules is hashed as it is"
    );
}

#[test]
fn editing_only_the_run_ledger_or_the_approval_log_leaves_the_identity_unchanged() {
    let repo = TempRepo::with_project("excluded");
    repo.write("other/runs.intent", "// not the catalog's ledger\n");
    let head = repo.commit("first");
    let source = repo.source();
    let identity = source.working_tree_identity().unwrap();
    assert_eq!(identity, source.commit_identity(&head).unwrap());

    repo.write("intent/runs.intent", "// runs\nrun { example: x }\n");
    repo.write("intent/approvals.intent", "// approvals\napproval { }\n");
    assert_eq!(source.working_tree_identity().unwrap(), identity);
    assert!(source.matches_head().unwrap());
    let recorded = repo.commit("record runs and approvals");
    assert_eq!(source.commit_identity(&recorded).unwrap(), identity);

    repo.write("other/runs.intent", "// edited\n");
    assert_ne!(source.working_tree_identity().unwrap(), identity);
}

#[test]
fn a_deleted_tracked_file_gives_the_identity_of_the_commit_that_removes_it() {
    let repo = TempRepo::with_project("deleted");
    repo.commit("first");
    fs::remove_file(repo.dir.join("README.md")).unwrap();
    let source = repo.source();
    let deleted = source.working_tree_identity().unwrap();
    assert!(!source.matches_head().unwrap());

    let removed = repo.commit("remove README");
    assert_eq!(source.commit_identity(&removed).unwrap(), deleted);
}

#[cfg(unix)]
#[test]
fn a_tracked_symlink_counts_as_its_link_text() {
    let repo = TempRepo::with_project("symlink");
    std::os::unix::fs::symlink("README.md", repo.dir.join("GUIDE.md")).unwrap();
    let head = repo.commit("link");
    let source = repo.source();
    assert_eq!(
        source.working_tree_identity().unwrap(),
        source.commit_identity(&head).unwrap()
    );

    fs::remove_file(repo.dir.join("GUIDE.md")).unwrap();
    std::os::unix::fs::symlink("src/lib.rs", repo.dir.join("GUIDE.md")).unwrap();
    assert!(
        !source.matches_head().unwrap(),
        "retargeting a link changes it"
    );
}

// Covers AE20 (data half)
#[test]
fn the_diff_against_a_base_holds_tracked_changes_without_the_ledger_or_approval_log() {
    let repo = TempRepo::with_project("diff");
    let base = repo.commit("base");
    let source = repo.source();
    assert_eq!(source.diff(&base).unwrap(), "");

    repo.write("src/lib.rs", "pub fn answer() -> u8 {\n    0\n}\n");
    repo.write("intent/runs.intent", "// runs\n// a new run\n");
    repo.write(
        "intent/approvals.intent",
        "// approvals\n// a new approval\n",
    );
    repo.write("untracked.txt", "scratch\n");
    let diff = source.diff(&base).unwrap();

    assert!(diff.contains("+++ b/src/lib.rs"), "{diff}");
    assert!(diff.contains("-    42\n+    0\n"), "{diff}");
    for left_out in ["runs.intent", "approvals.intent", "untracked.txt"] {
        assert!(!diff.contains(left_out), "{left_out} in {diff}");
    }
}

#[test]
fn git_failures_name_the_failing_command() {
    let repo = TempRepo::with_project("errors");
    repo.commit("first");
    let source = repo.source();
    let error = source.commit_identity("no-such-commit").unwrap_err();
    assert!(error.contains("git ls-tree"), "{error}");

    let outside = std::env::temp_dir().join(format!(
        "lemmaspec-intent-identity-outside-{}",
        std::process::id()
    ));
    fs::create_dir_all(&outside).unwrap();
    let error = Repo::of_catalog(&outside).unwrap_err();
    fs::remove_dir_all(&outside).unwrap();
    assert!(error.contains("git rev-parse"), "{error}");
}

#[test]
fn the_catalog_may_live_below_the_top_level() {
    let repo = TempRepo::new("nested");
    repo.write("src/lib.rs", "pub fn answer() -> u8 {\n    42\n}\n");
    repo.write("docs/intent/catalog.intent", "catalog fixture {\n}\n");
    repo.write("docs/intent/runs.intent", "// runs\n");
    let head = repo.commit("first");
    let source = Repo::of_catalog(&repo.dir.join("docs/intent")).unwrap();
    assert_eq!(
        fs::canonicalize(&source.root).unwrap(),
        fs::canonicalize(&repo.dir).unwrap()
    );
    let identity = source.working_tree_identity().unwrap();
    assert_eq!(identity, source.commit_identity(&head).unwrap());

    repo.write("docs/intent/runs.intent", "// runs\n// another\n");
    assert_eq!(source.working_tree_identity().unwrap(), identity);
}
