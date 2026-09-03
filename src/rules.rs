use crate::parser::CommitRecord;

pub struct Finding {
    pub line: usize,
    pub rule: &'static str,
    pub message: String,
}

const MAX_SUBJECT_LEN: usize = 72;
const MAX_BODY_LINE_LEN: usize = 100;

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

    let subject_len = subject.chars().count();
    if subject_len > MAX_SUBJECT_LEN {
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
