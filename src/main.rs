mod config;
mod parser;
mod rules;

use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::process::{Command, ExitCode, Stdio};

use config::{Config, DEFAULT_CONFIG_FILE};

fn usage() -> String {
    format!(
        "githist-lint - lint git commit history for message style issues\n\
         \n\
         USAGE:\n\
         \x20   git log --format='{format}' | githist-lint\n\
         \x20   githist-lint <file>\n\
         \x20   githist-lint --git [<git-log-args>...]\n\
         \n\
         The first two forms read commit records from stdin (or FILE) in\n\
         the field-separated format produced by the git log command above.\n\
         The third form runs that git log command for you, in the current\n\
         directory, passing any extra arguments straight through, e.g.\n\
         \n\
         \x20   githist-lint --git origin/main..HEAD\n\
         \x20   githist-lint --git --since=2024-01-01\n\
         \n\
         Every form prints one line per finding:\n\
         \n\
         \x20   <commit> <line>: [<rule>] <message>\n\
         \n\
         Input is streamed one commit record at a time, so history of any\n\
         length can be linted without loading it all into memory.\n\
         \n\
         If a {DEFAULT_CONFIG_FILE} file exists in the current directory,\n\
         it is read for rule thresholds and enable/disable settings, e.g.\n\
         \n\
         \x20   subject-too-long.max = 100\n\
         \x20   subject-not-imperative.enabled = false\n",
        format = parser::EXPECTED_FORMAT
    )
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{}", usage());
        return ExitCode::SUCCESS;
    }

    let config = match Config::load_default() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("githist-lint: {e}");
            return ExitCode::FAILURE;
        }
    };

    let outcome = match args.split_first() {
        Some((flag, git_args)) if flag == "--git" => run_git_log(git_args, &config),
        Some((path, _)) => File::open(path)
            .map(BufReader::new)
            .and_then(|r| run(r, &config)),
        None => run(io::stdin().lock(), &config),
    };

    match outcome {
        Ok(finding_count) if finding_count > 0 => ExitCode::FAILURE,
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("githist-lint: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Runs `git log --format=<EXPECTED_FORMAT> <git_args>` itself and streams
/// its stdout the same way piped input would be streamed, so callers don't
/// have to spell out the format string on their own command line. `git_args`
/// is passed straight through, so revision ranges, `--since`, path filters,
/// and the like all work exactly as they would with a plain `git log`.
fn run_git_log(git_args: &[String], config: &Config) -> io::Result<usize> {
    let mut child = Command::new("git")
        .arg("log")
        .arg(format!("--format={}", parser::EXPECTED_FORMAT))
        .args(git_args)
        .stdout(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().expect("child stdout was not piped");
    let finding_count = run(BufReader::new(stdout), config)?;

    let status = child.wait()?;
    if !status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("git log exited with {status}"),
        ));
    }

    Ok(finding_count)
}

fn run<R: BufRead>(reader: R, config: &Config) -> io::Result<usize> {
    let mut stream = parser::CommitStream::new(reader);
    let mut finding_count = 0;

    while let Some(commit) = stream.next_commit()? {
        for finding in rules::check(&commit, config) {
            println!(
                "{} {}: [{}] {}",
                short_hash(&commit.hash),
                finding.line,
                finding.rule,
                finding.message
            );
            finding_count += 1;
        }
    }

    Ok(finding_count)
}

fn short_hash(hash: &str) -> &str {
    &hash[..hash.len().min(10)]
}
