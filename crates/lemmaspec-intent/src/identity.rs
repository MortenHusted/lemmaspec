//! Source identity (KTD6): SHA-256 over the sorted path, mode and git blob ID
//! of every tracked file, leaving out the catalog's run ledger and approval
//! log. Working-tree blob IDs come from `git hash-object`, which applies the
//! repository's line-ending rules, so a clean checkout has exactly its commit
//! tree's identity on every platform.

use std::{
    collections::BTreeMap,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
};

use crate::{
    digest::sha256,
    loader::{APPROVALS_FILE, RUNS_FILE},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    pub root: PathBuf,
    catalog_prefix: String,
    ledger_and_log_paths: [String; 2],
}

impl Repo {
    pub fn of_catalog(catalog: &Path) -> Result<Repo, String> {
        let output = git(catalog, &["rev-parse", "--show-toplevel", "--show-prefix"])?;
        let mut lines = output.lines();
        let root = lines.next().unwrap_or_default();
        let prefix = lines.next().unwrap_or_default();
        Ok(Repo {
            root: root.into(),
            catalog_prefix: prefix.into(),
            ledger_and_log_paths: [RUNS_FILE, APPROVALS_FILE].map(|file| format!("{prefix}{file}")),
        })
    }

    /// The catalog file `name`, such as its approval log, as `commit` has it;
    /// `None` when that commit has no such file.
    pub fn catalog_file_at(&self, commit: &str, name: &str) -> Result<Option<String>, String> {
        let path = format!("{}{name}", self.catalog_prefix);
        let listed = git(&self.root, &["ls-tree", "--name-only", commit, "--", &path])?;
        if listed.is_empty() {
            return Ok(None);
        }
        git(&self.root, &["show", &format!("{commit}:{path}")]).map(Some)
    }

    /// The identity of the tracked files as they are in the working tree. A
    /// tracked file missing from the working tree is left out, as a commit
    /// that deletes it would leave it out.
    pub fn working_tree_identity(&self) -> Result<String, String> {
        let listing = git(&self.root, &["ls-files", "-s", "-z"])?;
        let mut modes = BTreeMap::new();
        for entry in records(&listing) {
            let (meta, path) = entry
                .split_once('\t')
                .ok_or_else(|| format!("unexpected `git ls-files -s -z` entry {entry:?}"))?;
            let mode = meta.split(' ').next().unwrap_or_default();
            if !self.is_excluded(path) {
                modes.insert(path, mode);
            }
        }
        let mut files = Vec::new();
        let mut blobs = BTreeMap::new();
        for &path in modes.keys() {
            match fs::symlink_metadata(self.root.join(path)) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    blobs.insert(path, self.link_blob(path)?);
                }
                Ok(_) => files.push(path),
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(format!("cannot read {path}: {e}")),
            }
        }
        let input: String = files
            .iter()
            .map(|path| format!("{}\n", c_quoted(path)))
            .collect();
        let hashed = git_with_input(
            &self.root,
            &["hash-object", "--stdin-paths"],
            input.into_bytes(),
        )?;
        let hashed: Vec<&str> = hashed.lines().collect();
        if hashed.len() != files.len() {
            return Err(format!(
                "`git hash-object --stdin-paths` hashed {} of {} files",
                hashed.len(),
                files.len()
            ));
        }
        blobs.extend(files.into_iter().zip(hashed.into_iter().map(String::from)));
        Ok(identity(
            blobs
                .iter()
                .map(|(path, blob)| (*path, modes[path], blob.as_str())),
        ))
    }

    pub fn commit_identity(&self, commit: &str) -> Result<String, String> {
        let listing = git(&self.root, &["ls-tree", "-r", "-z", commit])?;
        let mut entries = Vec::new();
        for entry in records(&listing) {
            let (meta, path) = entry
                .split_once('\t')
                .ok_or_else(|| format!("unexpected `git ls-tree -r -z` entry {entry:?}"))?;
            let mut meta = meta.split(' ');
            let (Some(mode), Some(_), Some(blob)) = (meta.next(), meta.next(), meta.next()) else {
                return Err(format!("unexpected `git ls-tree -r -z` entry {entry:?}"));
            };
            if !self.is_excluded(path) {
                entries.push((path, mode, blob));
            }
        }
        Ok(identity(entries))
    }

    pub fn head(&self) -> Result<String, String> {
        Ok(git(&self.root, &["rev-parse", "--verify", "HEAD"])?
            .trim()
            .to_string())
    }

    pub fn matches_head(&self) -> Result<bool, String> {
        Ok(self.working_tree_identity()? == self.commit_identity("HEAD")?)
    }

    /// The diff of the tracked files in the working tree against `base`,
    /// without the run ledger and the approval log.
    pub fn diff(&self, base: &str) -> Result<String, String> {
        let excluded = self
            .ledger_and_log_paths
            .each_ref()
            .map(|path| format!(":(exclude){path}"));
        let mut args = vec!["diff", "--no-color", "--no-ext-diff", base, "--", "."];
        args.extend(excluded.iter().map(String::as_str));
        git(&self.root, &args)
    }

    fn is_excluded(&self, path: &str) -> bool {
        self.ledger_and_log_paths
            .iter()
            .any(|excluded| excluded == path)
    }

    /// The blob ID of a symlink's own text, which `git hash-object <path>`
    /// would not give because it follows the link.
    fn link_blob(&self, path: &str) -> Result<String, String> {
        let target = fs::read_link(self.root.join(path))
            .map_err(|e| format!("cannot read the link {path}: {e}"))?;
        #[cfg(unix)]
        let text = std::os::unix::ffi::OsStrExt::as_bytes(target.as_os_str()).to_vec();
        #[cfg(not(unix))]
        let text = target.to_string_lossy().replace('\\', "/").into_bytes();
        Ok(
            git_with_input(&self.root, &["hash-object", "--stdin"], text)?
                .trim()
                .to_string(),
        )
    }
}

fn identity<'a>(entries: impl IntoIterator<Item = (&'a str, &'a str, &'a str)>) -> String {
    let lines: BTreeMap<&str, String> = entries
        .into_iter()
        .map(|(path, mode, blob)| (path, format!("{mode} {blob} {path}\n")))
        .collect();
    sha256(&lines.into_values().collect::<String>())
}

fn records(output: &str) -> impl Iterator<Item = &str> {
    output.split('\0').filter(|record| !record.is_empty())
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    run_git(dir, args, None)
}

fn git_with_input(dir: &Path, args: &[&str], input: Vec<u8>) -> Result<String, String> {
    run_git(dir, args, Some(input))
}

fn run_git(dir: &Path, args: &[&str], input: Option<Vec<u8>>) -> Result<String, String> {
    let command = format!("git {}", args.join(" "));
    let mut child = Command::new("git")
        .args(args)
        .current_dir(dir)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run `{command}`: {e}"))?;
    // Feed stdin from a thread so a large output cannot block the input.
    let writer = child.stdin.take().zip(input).map(|(mut stdin, input)| {
        thread::spawn(move || {
            let _ = stdin.write_all(&input);
        })
    });
    let output = child
        .wait_with_output()
        .map_err(|e| format!("`{command}` failed: {e}"))?;
    if let Some(writer) = writer {
        let _ = writer.join();
    }
    if !output.status.success() {
        return Err(format!(
            "`{command}` failed in {}: {}",
            dir.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// `git hash-object --stdin-paths` unquotes a line that starts with `"` and
/// trims a trailing carriage return, so every path is sent C-quoted.
fn c_quoted(path: &str) -> String {
    let mut quoted = String::from("\"");
    for byte in path.bytes() {
        match byte {
            b'"' => quoted.push_str("\\\""),
            b'\\' => quoted.push_str("\\\\"),
            b'\n' => quoted.push_str("\\n"),
            b'\r' => quoted.push_str("\\r"),
            b'\t' => quoted.push_str("\\t"),
            0x20..=0x7e => quoted.push(char::from(byte)),
            _ => quoted.push_str(&format!("\\{byte:03o}")),
        }
    }
    quoted.push('"');
    quoted
}
