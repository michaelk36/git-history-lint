use crate::parser::CommitRecord;

pub struct Finding {
    pub line: usize,
    pub rule: &'static str,
    pub message: String,
}

const MAX_SUBJECT_LEN: usize = 72;
const MAX_BODY_LINE_LEN: usize = 100;

// Base-form verbs that legitimately end in a single "s" and would
// otherwise be mistaken for a third-person conjugation (e.g. "Fixes").
const IMPERATIVE_S_EXCEPTIONS: &[&str] = &["focus", "canvas", "bias", "atlas"];

/// Guesses whether a commit subject opens with a non-imperative verb form
/// and, if so, returns the reason. This is a heuristic based on common
/// English inflection endings, not real grammatical analysis: it will
/// miss irregular verbs (e.g. "Made", "Ran") and can be fooled by nouns,
/// but it catches the vast majority of "Fixed bug" / "Adds feature" /
/// "Fixing typo" style subjects that style guides tell you to avoid.
fn non_imperative_reason(subject: &str) -> Option<String> {
    let token = subject.split_whitespace().next()?;
    let word = token.trim_matches(|c: char| !c.is_ascii_alphabetic());
    if word.len() < 3 {
        return None;
    }
    let lower = word.to_lowercase();

    if lower.ends_with("ing") {
        return Some(format!(
            "subject starts with \"{word}\", which looks like a gerund; use imperative mood (\"Add\", not \"Adding\")"
        ));
    }
    if lower.ends_with("ed") {
        return Some(format!(
            "subject starts with \"{word}\", which looks like past tense; use imperative mood (\"Add\", not \"Added\")"
        ));
    }
    if lower.ends_with('s')
        && !lower.ends_with("ss")
        && !IMPERATIVE_S_EXCEPTIONS.contains(&lower.as_str())
    {
        return Some(format!(
            "subject starts with \"{word}\", which looks like third person; use imperative mood (\"Add\", not \"Adds\")"
        ));
    }
    None
}

/// True for the subject git itself writes on `git revert`, e.g.
/// `Revert "Add feature"`. These wrap an already-reviewed subject in a
/// fixed template, so they aren't the author's free-form wording.
fn is_revert_subject(subject: &str) -> bool {
    subject.starts_with("Revert \"") && subject.ends_with('"')
}

pub fn check(commit: &CommitRecord) -> Vec<Finding> {
    let mut findings = Vec::new();

    let subject = commit.subject.trim();
    if subject.is_empty() {
        findings.push(Finding {
            line: commit.subject_line,
            rule: "empty-subject",
            message: "commit has no subject line".to_string(),
        });
        return findings;
    }

    // Merge commit subjects are written by git or by a hosting platform
    // (e.g. "Merge pull request #123 from org/some-long-branch-name"),
    // and revert subjects are a fixed wrapper around one that was already
    // linted when it was first committed. Holding either to the same
    // length/mood rules as an author-written subject produces findings
    // nobody can act on.
    let is_generated = commit.is_merge() || is_revert_subject(subject);

    let subject_len = subject.chars().count();
    if !is_generated && subject_len > MAX_SUBJECT_LEN {
        findings.push(Finding {
            line: commit.subject_line,
            rule: "subject-too-long",
            message: format!(
                "subject is {subject_len} characters, keep it under {MAX_SUBJECT_LEN}"
            ),
        });
    }

    if subject.ends_with('.') {
        findings.push(Finding {
            line: commit.subject_line,
            rule: "subject-trailing-period",
            message: "subject line should not end with a period".to_string(),
        });
    }

    if let Some(first) = subject.chars().next() {
        if first.is_lowercase() {
            findings.push(Finding {
                line: commit.subject_line,
                rule: "subject-not-capitalized",
                message: "subject line should start with a capital letter".to_string(),
            });
        }
    }

    if !is_generated {
        if let Some(reason) = non_imperative_reason(subject) {
            findings.push(Finding {
                line: commit.subject_line,
                rule: "subject-not-imperative",
                message: reason,
            });
        }
    }

    let mut line = commit.body_start_line;
    for body_line in commit.body.split('\n') {
        let len = body_line.chars().count();
        if len > MAX_BODY_LINE_LEN {
            findings.push(Finding {
                line,
                rule: "body-line-too-long",
                message: format!(
                    "body line is {len} characters, keep it under {MAX_BODY_LINE_LEN}"
                ),
            });
        }
        line += 1;
    }

    findings
}
