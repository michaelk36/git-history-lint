use std::io::{self, BufRead};

// git's pretty-format lets us insert raw bytes with %x<hex>. We use the
// ASCII record and unit separators as delimiters because real commit
// messages essentially never contain them, unlike commas, tabs, or even
// null bytes (which some tools use but which show up in binary-looking
// commit trailers more often than you'd expect).
const RECORD_SEP: u8 = 0x1e;
const FIELD_SEP: u8 = 0x1f;

/// The format string this parser expects on stdin:
///
///   git log --format='%x1e%H%x1f%P%x1f%an%x1f%ae%x1f%ad%x1f%s%x1f%b'
pub const EXPECTED_FORMAT: &str = "%x1e%H%x1f%P%x1f%an%x1f%ae%x1f%ad%x1f%s%x1f%b";

pub struct CommitRecord {
    pub hash: String,
    pub parent_hashes: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    pub date: String,
    pub subject: String,
    pub subject_line: usize,
    pub body: String,
    pub body_start_line: usize,
}

impl CommitRecord {
    /// A commit with more than one parent is a merge commit. Its subject
    /// is usually written by git or a hosting platform, not the author,
    /// so the usual style rules don't apply to it.
    pub fn is_merge(&self) -> bool {
        self.parent_hashes.len() > 1
    }
}

/// Pulls one commit record at a time off a reader, never holding more
/// than a single record in memory. `line` tracks the reader's position
/// in terms of newline-delimited lines so findings can point back at
/// roughly where they came from in the piped input.
pub struct CommitStream<R: BufRead> {
    reader: R,
    line: usize,
    buf: Vec<u8>,
}

impl<R: BufRead> CommitStream<R> {
    pub fn new(reader: R) -> Self {
        CommitStream { reader, line: 1, buf: Vec::new() }
    }

    pub fn next_commit(&mut self) -> io::Result<Option<CommitRecord>> {
        loop {
            self.buf.clear();
            let read = self.reader.read_until(RECORD_SEP, &mut self.buf)?;
            if read == 0 {
                return Ok(None);
            }
            if self.buf.last() == Some(&RECORD_SEP) {
                self.buf.pop();
            }

            let record_start_line = self.line;
            self.line += self.buf.iter().filter(|&&b| b == b'\n').count();

            if self.buf.is_empty() {
                // The format string opens with the record separator, so
                // the very first read_until call returns an empty chunk.
                continue;
            }

            return Ok(Some(parse_record(&self.buf, record_start_line)));
        }
    }
}

fn parse_record(buf: &[u8], record_start_line: usize) -> CommitRecord {
    let mut fields = buf.splitn(7, |&b| b == FIELD_SEP);
    let mut next_field = || fields.next().unwrap_or(&[] as &[u8]);

    let hash = field_to_string(next_field());
    let parent_hashes = field_to_string(next_field())
        .split_whitespace()
        .map(str::to_string)
        .collect();
    let author_name = field_to_string(next_field());
    let author_email = field_to_string(next_field());
    let date = field_to_string(next_field());

    let subject_bytes = next_field();
    let subject = field_to_string(subject_bytes);
    let subject_line = record_start_line;

    let body_bytes = next_field();
    let body_start_line =
        subject_line + subject_bytes.iter().filter(|&&b| b == b'\n').count();
    let body = String::from_utf8_lossy(body_bytes).into_owned();

    CommitRecord {
        hash,
        parent_hashes,
        author_name,
        author_email,
        date,
        subject,
        subject_line,
        body,
        body_start_line,
    }
}

fn field_to_string(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_string()
}
