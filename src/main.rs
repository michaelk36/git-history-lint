mod parser;
mod rules;

use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::process::ExitCode;

fn usage() -> String {
    format!(
        "githist-lint - lint git commit history for message style issues\n\
         \n\
         USAGE:\n\
         \x20   git log --format='{}' | githist-lint\n\
         \x20   githist-lint <file>\n\
         \n\
         Reads commit records from stdin (or FILE) in the field-separated\n\
         format produced by the git log command above and prints one line\n\
         per finding:\n\
         \n\
         \x20   <commit> <line>: [<rule>] <message>\n\
         \n\
         Input is streamed one commit record at a time, so history of any\n\
         length can be linted without loading it all into memory.\n",
        parser::EXPECTED_FORMAT
    )
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{}", usage());
        return ExitCode::SUCCESS;
    }

    let outcome = match args.first() {
        Some(path) => File::open(path).map(BufReader::new).and_then(run),
        None => run(io::stdin().lock()),
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

fn run<R: BufRead>(reader: R) -> io::Result<usize> {
    let mut stream = parser::CommitStream::new(reader);
    let mut finding_count = 0;

    while let Some(commit) = stream.next_commit()? {
        for finding in rules::check(&commit) {
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
