# githist-lint

A linter for git commit history. It checks commit messages against a small
set of style rules (subject line length, capitalization, trailing periods,
body line length) and reports findings with a commit hash and a line number,
the way a code linter reports file and line.

Commit message quality tends to drift once a repo has more than one
contributor: subject lines run past what `git log --oneline` can display,
some end in a period and some don't, bodies get pasted in as unwrapped
walls of text. None of that breaks anything, but it makes `git log` and
`git blame` annoying to read six months later. This is meant to run in CI
or a pre-push hook so that drift gets caught instead of accumulating.

## Why it reads from a pipe instead of shelling out to git itself

The tool never calls `git` itself and never touches `.git`. Instead it reads
commit records from stdin in a format you produce with `git log`. That
keeps the tool honest about memory: it processes one record at a time and
never buffers the whole history, so it works the same on a 50-commit repo
and a 500,000-commit one. It also means it works on any input that looks
like git log output, including a saved file, which is useful for testing.

The expected format uses two ASCII control characters (the record and unit
separators, `0x1e` and `0x1f`) as delimiters instead of something visible
like a comma or a pipe, because those are effectively guaranteed not to show
up inside a real commit message:

```
git log --format='%x1e%H%x1f%P%x1f%an%x1f%ae%x1f%ad%x1f%s%x1f%b' | githist-lint
```

You can also point it at a file that holds the same format:

```
git log --format='%x1e%H%x1f%P%x1f%an%x1f%ae%x1f%ad%x1f%s%x1f%b' > history.log
githist-lint history.log
```

## Output

One line per finding:

```
a1b2c3d4e5 1: [subject-too-long] subject is 84 characters, keep it under 72
a1b2c3d4e5 3: [body-line-too-long] body line is 143 characters, keep it under 100
f6e5d4c3b2 6: [subject-trailing-period] subject line should not end with a period
```

The line number refers to the position within the piped `git log` output,
not the original file the commit touched.

The process exits with status 1 if any findings were reported, 0 otherwise,
so it can be used directly as a CI gate.

## Rules

| rule                       | checks                                              |
|-----------------------------|------------------------------------------------------|
| `empty-subject`             | commit message has no subject line                   |
| `subject-too-long`          | subject line longer than 72 characters                |
| `subject-trailing-period`   | subject line ends with `.`                             |
| `subject-not-capitalized`   | subject line starts with a lowercase letter            |
| `subject-not-imperative`    | subject line starts with a likely gerund/past-tense/third-person verb |
| `body-line-too-long`        | a body line longer than 100 characters                 |

`subject-not-imperative` is a heuristic based on the first word's ending
(`-ing`, `-ed`, or a trailing `-s` that isn't part of the base word), not
real grammar. It will miss irregular verbs like "Made" or "Ran".

Merge commits (detected from `%P` having more than one parent hash) and
`git revert`'s `Revert "<original subject>"` wrapper are exempt from
`subject-too-long` and `subject-not-imperative`: that text comes from git
or a hosting platform, not the author, so flagging it doesn't help anyone.

## Config

If a `.githist-lint.toml` file exists in the current directory, it's read
for per-rule thresholds and enable/disable settings. Each line is a dotted
`rule-name.field = value` assignment (a small, hand-parsed subset of real
TOML — comments start with `#`, blank lines are ignored):

```
subject-too-long.max = 100
body-line-too-long.max = 120
subject-not-imperative.enabled = false
```

`max` is only valid on `subject-too-long` and `body-line-too-long`.
`enabled` is valid on any rule. An unknown rule name, an unknown field, or
a value that doesn't parse is an error rather than being ignored, so a
typo in the config doesn't quietly turn off a rule. With no config file
present, the defaults are a 72-character subject limit and a
100-character body line limit, with every rule enabled.

## Status

Early skeleton. The rule set above is intentionally small; more rules
(ticket reference conventions, for example) are easy to add in
`src/rules.rs` now that parsing, streaming, and config are in place. No
third-party dependencies are used or planned.

## License

MIT, see `LICENSE`.
