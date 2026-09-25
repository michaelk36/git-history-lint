// Integration tests for the compiled binary. The crate only has a [[bin]]
// target (no lib), so the only way to exercise main.rs, parser.rs, rules.rs
// and config.rs together is to spawn githist-lint and feed it fixture input
// in the same record/field-separated format `git log` would produce.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const RS: char = '\u{1e}';
const FS: char = '\u{1f}';

/// A 40-char hex-ish hash built by repeating `digit`, so the first 10
/// characters (what `short_hash` prints) are unambiguous per test commit.
fn hash(digit: char, tail: char) -> String {
    format!("{}{tail}", digit.to_string().repeat(39))
}

fn short(digit: char) -> String {
    digit.to_string().repeat(10)
}

/// One fixture commit record in the exact byte layout `CommitStream`
/// expects: a leading record separator, six field-separated header
/// fields, then subject and body, terminated by the newline git appends
/// after every commit's formatted output.
fn record(hash: &str, parents: &str, subject: &str, body: &str) -> String {
    format!(
        "{RS}{hash}{FS}{parents}{FS}Author Name{FS}author@example.com{FS}Mon Jan 1 00:00:00 2024 +0000{FS}{subject}{FS}{body}\n"
    )
}

struct RunResult {
    stdout: String,
    stderr: String,
    success: bool,
}

fn run(dir: &Path, input: &str) -> RunResult {
    let mut child = Command::new(env!("CARGO_BIN_EXE_githist-lint"))
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start githist-lint binary");

    child
        .stdin
        .take()
        .expect("child stdin was not piped")
        .write_all(input.as_bytes())
        .expect("failed to write fixture to child stdin");

    let output = child.wait_with_output().expect("failed to wait on child");
    RunResult {
        stdout: String::from_utf8(output.stdout).expect("stdout was not utf-8"),
        stderr: String::from_utf8(output.stderr).expect("stderr was not utf-8"),
        success: output.status.success(),
    }
}

/// A fresh directory per test, used as the child process's cwd, so tests
/// never depend on (or interfere with) whether a `.githist-lint.toml`
/// happens to sit in the repo root.
fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("githist-lint-test-{label}-{nanos}"));
    fs::create_dir_all(&dir).expect("failed to create temp dir");
    dir
}

#[test]
fn no_findings_for_well_formed_commit() {
    let dir = temp_dir("clean-commit");
    let input = record(
        &hash('d', '1'),
        &hash('e', '2'),
        "Add contributor guide",
        "Explains how to open a pull request and run the test suite before submitting.",
    );

    let result = run(&dir, &input);

    assert!(result.success, "stderr: {}", result.stderr);
    assert_eq!(result.stdout, "");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn reports_subject_too_long() {
    let dir = temp_dir("subject-too-long");
    let subject = format!("Refactor {}", "x".repeat(70));
    assert_eq!(subject.chars().count(), 79);
    let input = record(&hash('a', '1'), &hash('b', '2'), &subject, "");

    let result = run(&dir, &input);

    assert!(!result.success);
    assert_eq!(
        result.stdout,
        format!(
            "{} 1: [subject-too-long] subject is 79 characters, keep it under 72\n",
            short('a')
        )
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn reports_findings_across_multiple_commits_in_stream() {
    let dir = temp_dir("multi-commit-stream");
    let hash1 = hash('1', 'a');
    let hash2 = hash('2', 'b');
    let mut input = record(&hash1, "", "fix bug.", "");
    input += &record(&hash2, &hash1, "Add contributor guide", &"x".repeat(150));

    let result = run(&dir, &input);

    assert!(!result.success);
    assert_eq!(
        result.stdout,
        format!(
            "{h1} 1: [subject-trailing-period] subject line should not end with a period\n\
             {h1} 1: [subject-not-capitalized] subject line should start with a capital letter\n\
             {h2} 2: [body-line-too-long] body line is 150 characters, keep it under 100\n",
            h1 = short('1'),
            h2 = short('2'),
        )
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn empty_subject_short_circuits_other_rules() {
    let dir = temp_dir("empty-subject");
    let input = record(&hash('8', 'b'), &hash('9', 'c'), "", &"y".repeat(150));

    let result = run(&dir, &input);

    assert!(!result.success);
    assert_eq!(
        result.stdout,
        format!("{} 1: [empty-subject] commit has no subject line\n", short('8'))
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn merge_commit_is_exempt_from_length_and_mood_but_not_capitalization() {
    let dir = temp_dir("merge-commit");
    let subject =
        "merging release/2.4 into main with a lot of extra words to exceed the limit for sure";
    assert_eq!(subject.chars().count(), 84);
    let parents = format!("{} {}", hash('4', 'd'), hash('5', 'e'));
    let input = record(&hash('3', 'c'), &parents, subject, "");

    let result = run(&dir, &input);

    assert!(!result.success);
    assert_eq!(
        result.stdout,
        format!(
            "{} 1: [subject-not-capitalized] subject line should start with a capital letter\n",
            short('3')
        )
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn revert_commit_is_exempt_from_length_check() {
    let dir = temp_dir("revert-commit");
    let subject = "Revert \"Refactor the entire persistence layer to use a repository pattern for testability\"";
    assert_eq!(subject.chars().count(), 90);
    let input = record(&hash('6', 'f'), &hash('7', 'a'), subject, "");

    let result = run(&dir, &input);

    assert!(result.success, "stderr: {}", result.stderr);
    assert_eq!(result.stdout, "");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn config_file_overrides_threshold_and_disables_rule() {
    let dir = temp_dir("config-override");
    fs::write(
        dir.join(".githist-lint.toml"),
        "subject-too-long.max = 10\nsubject-not-capitalized.enabled = false\n",
    )
    .expect("failed to write config fixture");
    let subject = "add a small fix";
    assert_eq!(subject.chars().count(), 15);
    let input = record(&hash('f', '1'), &hash('c', '2'), subject, "");

    let result = run(&dir, &input);

    assert!(!result.success);
    assert_eq!(
        result.stdout,
        format!(
            "{} 1: [subject-too-long] subject is 15 characters, keep it under 10\n",
            short('f')
        )
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn git_flag_runs_git_log_itself() {
    let dir = temp_dir("git-flag");
    let git = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(&dir)
            .status()
            .expect("failed to run git");
        assert!(status.success(), "git {args:?} failed");
    };

    git(&["init", "--quiet"]);
    git(&["config", "user.name", "Author Name"]);
    git(&["config", "user.email", "author@example.com"]);
    git(&["commit", "--quiet", "--allow-empty", "-m", "fix bug."]);

    let mut child = Command::new(env!("CARGO_BIN_EXE_githist-lint"))
        .arg("--git")
        .current_dir(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start githist-lint binary");
    let output = child.wait_with_output().expect("failed to wait on child");
    let stdout = String::from_utf8(output.stdout).expect("stdout was not utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr was not utf-8");

    assert!(!output.status.success(), "stderr: {stderr}");
    assert!(
        stdout.contains("[subject-trailing-period]"),
        "stdout was: {stdout}"
    );
    assert!(
        stdout.contains("[subject-not-capitalized]"),
        "stdout was: {stdout}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn invalid_config_file_is_reported_as_an_error() {
    let dir = temp_dir("config-invalid");
    fs::write(dir.join(".githist-lint.toml"), "nonsense-rule.max = 5\n")
        .expect("failed to write config fixture");

    let result = run(&dir, "");

    assert!(!result.success);
    assert!(
        result.stderr.contains("unknown rule \"nonsense-rule\""),
        "stderr was: {}",
        result.stderr
    );
    let _ = fs::remove_dir_all(&dir);
}
