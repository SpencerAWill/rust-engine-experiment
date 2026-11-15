//! `cargo xtask guard-push`: a Claude Code `PreToolUse` hook that blocks `git push` to `main` and
//! force pushes.
//!
//! The hook JSON arrives on stdin. Exit 0 allows the command; exit 2 blocks it and the reason on
//! stderr goes to the agent. Input the guard cannot understand is blocked: a guard that fails open
//! would be no guard. The maintainer's own `! git push ...` commands never pass through hooks.

use std::io::Read;
use std::process::ExitCode;

/// What the guard decides about one Bash command.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Decision {
    /// Let the command run.
    Allow,
    /// Refuse it; the string says why and what the maintainer can do instead.
    Block(String),
}

/// Why a command with a quote that never closes is blocked.
const UNTERMINATED: &str = "the command has an unterminated quote";

/// Why any force push is blocked.
const FORCE: &str = "force pushes are not allowed";

/// Why a bare push or a `HEAD` refspec is blocked when the branch is not known.
const UNKNOWN_BRANCH: &str = "cannot tell which branch `HEAD`/a bare push sends";

/// Marks a segment that stands for the body of a heredoc that feeds a shell.
const BODY_MARK: char = '\u{1}';

/// Decides whether `command` may run. `current_branch` is the checked-out branch of the hook's
/// working directory, when known; a push with no explicit refspec pushes it.
pub(crate) fn decide(command: &str, current_branch: Option<&str>) -> Decision {
    match check_script(command, current_branch) {
        Ok(()) => Decision::Allow,
        Err(reason) => Decision::Block(blocked(&reason)),
    }
}

/// Checks a script, segment by segment, tracking the branch as it changes; `Err` is the reason
/// (without the "blocked" prefix, so nested scripts do not wrap it twice).
fn check_script(command: &str, current_branch: Option<&str>) -> Result<(), String> {
    let (stripped, shell_bodies) = strip_heredocs(command);
    let segments = match split_segments(&stripped) {
        Ok(segments) => segments,
        // Fail closed only when the text looks like a push; a command whose quoting the simple
        // parser cannot follow, and that never says `git ... push`, is not the guard's business.
        Err(why) if mentions_git_push(&stripped) => return Err(why),
        Err(_) => return Ok(()),
    };
    let mut branch = current_branch;
    for segment in &segments {
        let words = &segment.words;
        // A heredoc that feeds a shell is a script: decide on it where it runs.
        if let [word] = words.as_slice()
            && let Some(index) = word.strip_prefix(BODY_MARK)
        {
            if let Some(body) = index
                .parse::<usize>()
                .ok()
                .and_then(|n| shell_bodies.get(n))
            {
                check_script(body, branch)?;
            }
            continue;
        }
        // Command substitutions are nested commands.
        for inner in substitutions(&segment.raw) {
            check_script(&inner, branch)?;
        }
        let command_at = locate(words);
        let git_env = words[..command_at.unwrap_or(words.len())]
            .iter()
            .any(|w| is_git_env(w));
        let here = if git_env { None } else { branch };
        if let Some(reason) = check_segment(words, &segment.here_strings, here) {
            return Err(reason);
        }
        if git_env || changes_branch(words, command_at) {
            branch = None;
        }
    }
    Ok(())
}

/// True for an assignment that points git at another repository.
fn is_git_env(word: &str) -> bool {
    word.starts_with("GIT_DIR=") || word.starts_with("GIT_WORK_TREE=")
}

/// True when the segment may change the checked-out branch or repository for what follows.
fn changes_branch(words: &[String], command_at: Option<usize>) -> bool {
    let Some(k) = command_at else {
        return false;
    };
    let rest = &words[k..];
    match basename(&rest[0]) {
        "cd" | "pushd" | "popd" => true,
        "export" | "declare" | "typeset" => rest.iter().any(|w| is_git_env(w)),
        "git" => matches!(
            git_subcommand(rest),
            Some("checkout" | "switch" | "worktree" | "rebase")
        ),
        _ => false,
    }
}

/// The subcommand of a `git ...` command (`rest[0]` is the program), after global options.
fn git_subcommand(rest: &[String]) -> Option<&str> {
    let mut j = 1;
    while let Some(w) = rest.get(j).filter(|w| w.starts_with('-')) {
        j += if matches!(
            w.as_str(),
            "-c" | "-C" | "--git-dir" | "--work-tree" | "--namespace"
        ) {
            2
        } else {
            1
        };
    }
    rest.get(j).map(String::as_str)
}

/// Decides from the raw hook JSON; `branch_of` maps the hook's `cwd` to its checked-out branch.
fn decide_hook_input(input: &str, branch_of: impl Fn(&str) -> Option<String>) -> Decision {
    let value: serde_json::Value = match serde_json::from_str(input) {
        Ok(value) => value,
        Err(e) => {
            return Decision::Block(blocked(&format!("the hook input is not valid JSON ({e})")));
        }
    };
    let Some(command) = value["tool_input"]["command"].as_str() else {
        return Decision::Block(blocked(
            "the hook input has no string field `tool_input.command`",
        ));
    };
    let branch = value["cwd"].as_str().and_then(branch_of);
    decide(command, branch.as_deref())
}

/// True when some line of `text` has the word `git` and, later, the word `push`.
fn mentions_git_push(text: &str) -> bool {
    text.lines().any(|line| {
        let mut words = line
            .split(|c: char| !(c.is_alphanumeric() || matches!(c, '-' | '_' | '.')))
            .filter(|w| !w.is_empty());
        words.any(|w| w == "git") && words.any(|w| w == "push")
    })
}

/// Shells whose `-c` script, or whose standard input, is a command script.
const SHELLS: [&str; 5] = ["sh", "bash", "zsh", "dash", "ksh"];

/// The last path component of a program word: `/usr/bin/git` is `git`.
fn basename(word: &str) -> &str {
    word.rsplit('/').next().unwrap_or(word)
}

/// True when some command on `line` runs a shell (found the way [`locate`] finds commands), so a
/// heredoc on that line may feed it (`bash <<EOF`, `cat <<EOF | sh`, `timeout 9 bash <<EOF`).
fn line_runs_shell(line: &str) -> bool {
    line.split([';', '|', '&', '(', '`']).any(|segment| {
        let words: Vec<String> = segment.split_whitespace().map(str::to_string).collect();
        locate(&words).is_some_and(|k| SHELLS.contains(&basename(&words[k])))
    })
}

/// True when the `#` at `i` starts a comment: it is at the start of a word.
fn comment_starts(chars: &[char], i: usize) -> bool {
    i == 0 || chars[i - 1].is_whitespace() || ";&|()".contains(chars[i - 1])
}

/// Parses a heredoc delimiter starting at `j`; returns it and the index after it.
fn heredoc_word(chars: &[char], mut j: usize) -> (String, usize) {
    let mut word = String::new();
    while let Some(&w) = chars.get(j) {
        if w == '\'' || w == '"' {
            j += 1;
            while let Some(&q) = chars.get(j) {
                j += 1;
                if q == w {
                    break;
                }
                word.push(q);
            }
        } else if w == '\\' {
            j += 1;
        } else if w.is_whitespace() || ";|&()<>".contains(w) {
            break;
        } else {
            word.push(w);
            j += 1;
        }
    }
    (word, j)
}

/// Removes heredoc operators and bodies (`<<WORD`, `<<'WORD'`, `<<"WORD"`, `<<-WORD`) and comments
/// from `command`; they are data. Returns the rest, and the bodies of heredocs on a line that runs
/// a shell, which are commands; the rest holds a [`BODY_MARK`] line where each such body ran.
/// Quotes are tracked so a `<<` inside a string is not an operator;
/// a here-string (`<<<`) is not a heredoc.
#[allow(clippy::too_many_lines)] // one scanner with quote, comment, arithmetic and heredoc states
fn strip_heredocs(command: &str) -> (String, Vec<String>) {
    let chars: Vec<char> = command.chars().collect();
    let mut out = String::new();
    let mut shell_bodies = Vec::new();
    // Heredocs opened on the current line: (delimiter, tabs allowed).
    let mut pending: Vec<(String, bool)> = Vec::new();
    let mut quote: Option<char> = None;
    // Parenthesis depth inside `((...))` or `$((...))`, where `<<` is a shift, not a heredoc.
    let mut arith = 0_usize;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match quote {
            Some(q) => {
                if c == '\\' && q == '"' {
                    out.push(c);
                    out.extend(chars.get(i + 1));
                    i += 2;
                    continue;
                }
                if c == q {
                    quote = None;
                }
            }
            None => match c {
                '#' if comment_starts(&chars, i) => {
                    while i < chars.len() && chars[i] != '\n' {
                        i += 1;
                    }
                    continue;
                }
                '\'' | '"' => quote = Some(c),
                '\\' => {
                    out.push(c);
                    out.extend(chars.get(i + 1));
                    i += 2;
                    continue;
                }
                '(' if arith > 0 => arith += 1,
                '(' if chars.get(i + 1) == Some(&'(') => {
                    arith = 2;
                    out.push_str("((");
                    i += 2;
                    continue;
                }
                ')' if arith > 0 => arith -= 1,
                '<' if arith == 0 && chars.get(i + 1) == Some(&'<') => {
                    if chars.get(i + 2) == Some(&'<') {
                        out.push_str("<<<");
                        i += 3;
                        continue;
                    }
                    let mut j = i + 2;
                    let dash = chars.get(j) == Some(&'-');
                    j += usize::from(dash);
                    while matches!(chars.get(j), Some(' ' | '\t')) {
                        j += 1;
                    }
                    let (word, end) = heredoc_word(&chars, j);
                    if !word.is_empty() {
                        pending.push((word, dash));
                        i = end;
                        continue;
                    }
                }
                '\n' if !pending.is_empty() => {
                    let shell = line_runs_shell(out.rsplit('\n').next().unwrap_or_default());
                    out.push('\n');
                    i += 1;
                    for (word, dash) in std::mem::take(&mut pending) {
                        let mut body = String::new();
                        while i < chars.len() {
                            let end = chars[i..]
                                .iter()
                                .position(|&x| x == '\n')
                                .map_or(chars.len(), |p| i + p);
                            let line: String = chars[i..end].iter().collect();
                            i = (end + 1).min(chars.len());
                            let test = if dash {
                                line.trim_start_matches('\t')
                            } else {
                                &line
                            };
                            if test == word {
                                break;
                            }
                            body.push_str(&line);
                            body.push('\n');
                        }
                        if shell {
                            out.push(BODY_MARK);
                            out.push_str(&shell_bodies.len().to_string());
                            out.push('\n');
                            shell_bodies.push(body);
                        }
                    }
                    continue;
                }
                _ => {}
            },
        }
        out.push(c);
        i += 1;
    }
    (out, shell_bodies)
}

/// The text of every command substitution (`$(...)` and backticks) in `text`, outside single
/// quotes; each is a nested command. Inner substitutions are found when the inner text is decided.
fn substitutions(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut found = Vec::new();
    let (mut single, mut double) = (false, false);
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if single {
            single = c != '\'';
            i += 1;
            continue;
        }
        match c {
            '#' if !double && comment_starts(&chars, i) => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            '\\' => {
                i += 2;
                continue;
            }
            '\'' if !double => single = true,
            '"' => double = !double,
            '`' => {
                let mut j = i + 1;
                while j < chars.len() && chars[j] != '`' {
                    j += if chars[j] == '\\' { 2 } else { 1 };
                }
                found.push(chars[i + 1..j.min(chars.len())].iter().collect());
                i = j + 1;
                continue;
            }
            '$' if chars.get(i + 1) == Some(&'(') => {
                let (mut depth, mut j) = (0_usize, i + 1);
                while j < chars.len() {
                    match chars[j] {
                        '\\' => j += 1,
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    j += 1;
                }
                found.push(chars[i + 2..j.min(chars.len())].iter().collect());
                i = j + 1;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    found
}

/// One simple command of a script, as [`split_segments`] cuts it.
#[derive(Debug, Default)]
struct Segment {
    /// The words, unquoted, without redirections.
    words: Vec<String>,
    /// The source text of the segment, quotes included, to look for substitutions in.
    raw: String,
    /// The words of here-strings (`<<< word`): standard input, which a shell runs as a script.
    here_strings: Vec<String>,
}

/// Accumulates the words and segments of [`split_segments`].
#[derive(Default)]
struct Splitter {
    segments: Vec<Segment>,
    words: Vec<String>,
    here_strings: Vec<String>,
    word: String,
    in_word: bool,
    /// Set by a redirection operator: the next word is its target and is not an argument.
    drop_next: bool,
    /// Set with `drop_next` by `<<<`: the target is kept as a here-string.
    here_next: bool,
    /// Index in the source where the current segment starts.
    start: usize,
}

impl Splitter {
    fn end_word(&mut self) {
        if self.in_word {
            let word = std::mem::take(&mut self.word);
            self.in_word = false;
            let here = std::mem::take(&mut self.here_next);
            if !std::mem::take(&mut self.drop_next) {
                self.words.push(word);
            } else if here {
                self.here_strings.push(word);
            }
        }
    }

    /// Ends the segment whose source is `chars[start..at]`.
    fn end_segment(&mut self, chars: &[char], at: usize) {
        self.end_word();
        self.drop_next = false;
        self.here_next = false;
        let raw: String = chars
            .get(self.start..at)
            .unwrap_or_default()
            .iter()
            .collect();
        if !self.words.is_empty() || !self.here_strings.is_empty() || !raw.trim().is_empty() {
            self.segments.push(Segment {
                words: std::mem::take(&mut self.words),
                raw,
                here_strings: std::mem::take(&mut self.here_strings),
            });
        }
        self.start = at + 1;
    }
}

/// Splits a shell command into segments (on `&&`, `||`, `;`, `|`, `&`, newlines and parentheses
/// outside quotes) and each segment into words, with single and double quotes and backslash
/// escapes respected, and comments (`#` at the start of a word, to the end of the line) dropped.
/// Redirections (`2>&1`, `> f`, `>> f`, `< f`, `&> f`) and their target words are dropped: they
/// are not arguments; the target of `<<<` is kept apart. An unterminated quote is an error.
fn split_segments(command: &str) -> Result<Vec<Segment>, String> {
    let mut s = Splitter::default();
    let chars: Vec<char> = command.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        i += 1;
        match c {
            '#' if !s.in_word => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '\'' => {
                s.in_word = true;
                loop {
                    match chars.get(i) {
                        Some('\'') => break,
                        Some(&q) => s.word.push(q),
                        None => return Err(UNTERMINATED.to_string()),
                    }
                    i += 1;
                }
                i += 1;
            }
            '"' => {
                s.in_word = true;
                loop {
                    match chars.get(i) {
                        Some('"') => break,
                        Some('\\') => {
                            i += 1;
                            s.word.extend(chars.get(i));
                        }
                        Some(&q) => s.word.push(q),
                        None => return Err(UNTERMINATED.to_string()),
                    }
                    i += 1;
                }
                i += 1;
            }
            '\\' => {
                s.in_word = true;
                s.word.extend(chars.get(i));
                i += 1;
            }
            '>' | '<' | '&' if c != '&' || chars.get(i) == Some(&'>') => {
                // A bare number before the operator is its file descriptor.
                if s.in_word && !s.word.is_empty() && s.word.chars().all(|d| d.is_ascii_digit()) {
                    s.word.clear();
                    s.in_word = false;
                } else {
                    s.end_word();
                }
                if chars.get(i) == Some(&'(') {
                    continue; // process substitution: the parenthesis splits
                }
                let operators = i;
                while matches!(chars.get(i), Some('<' | '>' | '&' | '|')) {
                    i += 1;
                }
                s.drop_next = true;
                s.here_next = c == '<' && chars[operators..i] == ['<', '<'];
            }
            ';' | '|' | '&' | '\n' | '(' | ')' => s.end_segment(&chars, i - 1),
            c if c.is_whitespace() => s.end_word(),
            c => {
                s.in_word = true;
                s.word.push(c);
            }
        }
    }
    s.end_segment(&chars, chars.len());
    Ok(s.segments)
}

/// Keywords that may start a segment before its command.
const KEYWORDS: [&str; 9] = [
    "if", "then", "elif", "else", "while", "until", "do", "!", "{",
];

/// For a wrapper program that runs the next command, the flags that take a value.
fn wrapper_flags(program: &str) -> Option<&'static [&'static str]> {
    Some(match program {
        "env" => &["-u", "-C"],
        "sudo" => &["-u", "-g", "-h", "-p", "-C", "-D", "-R", "-T", "-U"],
        "nice" => &["-n", "--adjustment"],
        "ionice" => &["-c", "-n", "-p"],
        "timeout" => &["-s", "-k", "--signal", "--kill-after"],
        "xargs" => &["-n", "-I", "-P", "-L", "-s", "-E", "-d", "-a", "-J"],
        "caffeinate" => &["-t", "-w"],
        "watch" => &["-n", "--interval"],
        "command" | "exec" | "nohup" | "time" | "setsid" | "stdbuf" | "builtin" => &[],
        _ => return None,
    })
}

/// The index of the command word of a segment, after keywords, assignments and wrappers
/// (`if ! timeout 5 env -i A=1 git ...`).
fn locate(words: &[String]) -> Option<usize> {
    let mut i = 0;
    loop {
        let w = words.get(i)?;
        if KEYWORDS.contains(&w.as_str()) || (w.contains('=') && !w.starts_with('-')) {
            i += 1;
            continue;
        }
        let program = basename(w);
        let Some(arg_flags) = wrapper_flags(program) else {
            return Some(i);
        };
        i += 1;
        while let Some(flag) = words.get(i).filter(|f| f.starts_with('-') && f.len() > 1) {
            i += if arg_flags.contains(&flag.as_str()) {
                2
            } else {
                1
            };
        }
        if program == "timeout" {
            i += 1; // the duration
        }
    }
}

/// Decides a nested script; returns the reason when it is blocked.
fn nested(script: &str, current_branch: Option<&str>) -> Option<String> {
    check_script(script, current_branch).err()
}

/// Why `gh` may not merge past the required checks.
const GH_MERGE: &str = "merging without the required checks is not allowed for agents";

/// Why a push through `xargs` is blocked.
const XARGS: &str = "a push through `xargs`: arguments are not known until the command runs";

/// Returns why a segment must be blocked, or `None` when it is not a risky push.
fn check_segment(
    words: &[String],
    here_strings: &[String],
    current_branch: Option<&str>,
) -> Option<String> {
    let at = locate(words)?;
    let rest = &words[at..];
    match basename(rest.first()?) {
        // `bash -lc "git push origin main"`: look inside the script.
        shell if SHELLS.contains(&shell) => {
            // `bash <<< 'git push origin main'`: the here-string is the script.
            for script in here_strings {
                if let Some(reason) = nested(script, current_branch) {
                    return Some(reason);
                }
            }
            let flag = rest
                .iter()
                .position(|w| w.starts_with('-') && !w.starts_with("--") && w.contains('c'))?;
            // The script is the first word after the `-c` cluster that is not an option.
            let script = rest[flag + 1..].iter().find(|w| !w.starts_with('-'))?;
            nested(script, current_branch)
        }
        "eval" => nested(&rest[1..].join(" "), current_branch),
        "git" => {
            let via_xargs = words[..at].iter().any(|w| basename(w) == "xargs");
            check_git(rest, via_xargs, current_branch)
        }
        "gh" => check_gh(rest),
        _ => None,
    }
}

/// Checks a `gh ...` command: no `gh pr merge --admin`, and no write call to a `.../merge` path.
fn check_gh(rest: &[String]) -> Option<String> {
    let positionals: Vec<&str> = rest[1..]
        .iter()
        .map(String::as_str)
        .filter(|w| !w.starts_with('-'))
        .collect();
    if positionals.contains(&"pr") && positionals.contains(&"merge") {
        let admin = rest
            .iter()
            .any(|w| w == "--admin" || w.starts_with("--admin="));
        return admin.then(|| GH_MERGE.to_string());
    }
    if positionals.first() != Some(&"api") {
        return None;
    }
    let merges = positionals.iter().any(|p| {
        p.split('?')
            .next()
            .is_some_and(|path| path.ends_with("/merge"))
    });
    let mut writes = false;
    let mut args = rest.iter().map(String::as_str);
    while let Some(arg) = args.next() {
        match arg {
            "-X" | "--method" => {
                writes |= args.next().is_some_and(|m| !m.eq_ignore_ascii_case("GET"));
            }
            // Fields and input make `gh api` post.
            "-f" | "-F" | "--field" | "--raw-field" | "--input" => {
                writes = true;
                args.next();
            }
            other => {
                let value = other
                    .strip_prefix("--method=")
                    .or_else(|| other.strip_prefix("-X"));
                writes |= value.is_some_and(|m| !m.eq_ignore_ascii_case("GET"));
            }
        }
    }
    (merges && writes).then(|| GH_MERGE.to_string())
}

/// Keys of `git -c` that can reroute a push.
const REROUTING_PREFIXES: [&str; 4] = ["remote.", "alias.", "push.", "url."];

/// Checks a `git ...` command (`rest[0]` is the program): global options, then `push` or `config`.
fn check_git(rest: &[String], via_xargs: bool, current_branch: Option<&str>) -> Option<String> {
    let mut j = 1;
    let mut redirected = false;
    while let Some(w) = rest.get(j) {
        match w.as_str() {
            "-c" => {
                if let Some(setting) = rest.get(j + 1) {
                    let key = setting.split('=').next().unwrap_or_default().to_lowercase();
                    if REROUTING_PREFIXES.iter().any(|p| key.starts_with(p)) {
                        return Some(format!("`git -c {setting}` could reroute pushes"));
                    }
                }
                j += 2;
            }
            "-C" | "--git-dir" | "--work-tree" | "--namespace" => {
                redirected = true;
                j += 2;
            }
            w if w.starts_with("--git-dir=") || w.starts_with("--work-tree=") => {
                redirected = true;
                j += 1;
            }
            w if w.starts_with('-') => j += 1,
            _ => break,
        }
    }
    match rest.get(j).map(String::as_str) {
        Some("push") if via_xargs => Some(XARGS.to_string()),
        Some("push") => check_push(
            &rest[j + 1..],
            if redirected { None } else { current_branch },
        ),
        Some("config") => check_config(&rest[j + 1..]),
        _ => None,
    }
}

/// Checks the arguments after `git config`: a write that could reroute pushes is blocked.
fn check_config(args: &[String]) -> Option<String> {
    let mut positionals: Vec<&str> = Vec::new();
    let mut skip_next = false;
    for arg in args {
        if std::mem::take(&mut skip_next) {
            continue;
        }
        match arg.as_str() {
            "-f" | "--file" | "--blob" | "--type" | "--default" | "--comment" => skip_next = true,
            "--get" | "--get-all" | "--get-regexp" | "--get-urlmatch" | "--list" | "-l"
            | "--unset" | "--unset-all" => return None,
            flag if flag.starts_with('-') => {}
            positional => positionals.push(positional),
        }
    }
    if positionals.first() == Some(&"set") {
        positionals.remove(0);
    }
    let key = positionals.first()?.to_lowercase();
    let rewrites = key.starts_with("alias.")
        || key.starts_with("push.")
        || key.starts_with("url.")
        || (key.starts_with("remote.")
            && matches!(key.rsplit('.').next(), Some("push" | "mirror")));
    (positionals.len() >= 2 && rewrites).then(|| format!("`git config {key}` would reroute pushes"))
}

/// What an option (an exact or abbreviated `--name`, without `=value`) means to the guard.
fn long_option(name: &str) -> Option<String> {
    if name.len() < 4 {
        return None;
    }
    let is = |full: &str| full.starts_with(name);
    if is("--force") || is("--force-with-lease") || is("--force-if-includes") {
        Some(FORCE.to_string())
    } else if is("--mirror") || is("--all") {
        Some(format!("`git push {name}` pushes every branch"))
    } else if is("--prune") {
        Some(format!("`git push {name}` deletes remote branches"))
    } else {
        None
    }
}

/// Checks the arguments after `git push`.
fn check_push(args: &[String], current_branch: Option<&str>) -> Option<String> {
    let mut positionals: Vec<&str> = Vec::new();
    let mut skip_next = false;
    for arg in args {
        if std::mem::take(&mut skip_next) {
            continue;
        }
        match arg.as_str() {
            "-o" | "--push-option" | "--repo" | "--receive-pack" | "--exec" => skip_next = true,
            long if long.starts_with("--") && long.len() > 2 => {
                if let Some(reason) = long_option(long.split('=').next().unwrap_or(long)) {
                    return Some(reason);
                }
            }
            short if short.starts_with('-') && short.len() > 1 && short != "--" => {
                if short.contains('f') {
                    return Some(FORCE.to_string());
                }
            }
            positional => positionals.push(positional),
        }
    }
    let refspecs = positionals.get(1..).unwrap_or_default();
    if refspecs.is_empty() {
        return match current_branch {
            Some("main") => Some("this pushes `main`".to_string()),
            Some(_) => None,
            None => Some(UNKNOWN_BRANCH.to_string()),
        };
    }
    for spec in refspecs {
        if spec.contains(['$', '`']) {
            return Some(format!("`{spec}` is not known until the command runs"));
        }
        if spec.starts_with('+') {
            return Some(format!("`{spec}` is a force push"));
        }
        let destination = spec.rsplit(':').next().unwrap_or(spec);
        let (destination, prefixed) = match destination
            .strip_prefix("refs/heads/")
            .or_else(|| destination.strip_prefix("heads/"))
        {
            Some(name) => (name, true),
            None => (destination, false),
        };
        let destination = if matches!(destination, "HEAD" | "@") {
            match current_branch {
                Some(branch) => branch,
                None => return Some(UNKNOWN_BRANCH.to_string()),
            }
        } else {
            destination
        };
        if destination == "main" || (prefixed && destination.ends_with("/main")) {
            return Some(format!("`{spec}` pushes to `main`"));
        }
    }
    None
}

/// Entry point: reads the hook JSON from stdin and exits 0 (allow) or 2 (block).
pub(crate) fn run() -> ExitCode {
    let mut input = String::new();
    let read = std::io::stdin().read_to_string(&mut input);
    let decision = match read {
        Ok(_) => decide_hook_input(&input, current_branch),
        Err(e) => Decision::Block(blocked(&format!("could not read the hook input: {e}"))),
    };
    match decision {
        Decision::Allow => ExitCode::SUCCESS,
        Decision::Block(reason) => {
            eprintln!("{reason}");
            ExitCode::from(2)
        }
    }
}

/// The checked-out branch of the repository at `cwd`, or `None` when git cannot say.
fn current_branch(cwd: &str) -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["-C", cwd, "rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn blocked(reason: &str) -> String {
    format!(
        "guard-push blocked this command: {reason}. Pushes to `main` and force pushes are not \
         allowed for agents. The maintainer can run the push themselves with `! git push ...`."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_block(command: &str, branch: Option<&str>) -> bool {
        matches!(decide(command, branch), Decision::Block(_))
    }

    #[test]
    fn blocks() {
        let cases: &[(&str, Option<&str>)] = &[
            ("git push origin main", None),
            ("git push origin HEAD:main", None),
            ("git push origin x:refs/heads/main", None),
            ("git push origin refs/heads/main", None),
            ("git push --force origin x", None),
            ("git push -f origin x", None),
            ("git push -uf origin x", None),
            ("git push --force-with-lease origin x", None),
            ("git push --force-with-lease=x:abc origin x", None),
            ("git push origin +x", None),
            ("git push --all origin", None),
            ("git push --mirror origin", None),
            ("git push origin --delete main", None),
            ("git push origin :main", None),
            ("git add a && git push origin main", None),
            ("cd r; git push origin main", None),
            ("git -C /r push origin main", None),
            ("git -c a=b push origin main", None),
            ("(cd r && git push origin main)", None),
            ("git push", Some("main")),
            ("git push origin", Some("main")),
            ("git push \"origin main", None),
        ];
        for (command, branch) in cases {
            assert!(is_block(command, *branch), "should block: {command}");
        }
    }

    #[test]
    fn allows() {
        let cases: &[(&str, Option<&str>)] = &[
            ("git push -u origin m3.15-agent-workflow", None),
            ("git push", Some("m3.15-agent-workflow")),
            ("git push origin maintenance", None),
            ("git push origin main-docs", None),
            ("git status", None),
            ("echo \"git push origin main\"", None),
            ("git commit -m \"push to main\"", None),
            ("git log --grep push", None),
        ];
        for (command, branch) in cases {
            assert_eq!(
                decide(command, *branch),
                Decision::Allow,
                "should allow: {command}"
            );
        }
    }

    /// The shape that tripped the guard in the step 6 trial.
    fn trial_command() -> String {
        let python = "python3 - <<'EOF'\nimport re\ns = open('f').read()\nprint(s.replace(\"a\",\"b\"))\nEOF\n";
        let prose = "cat >> f <<'EOF'\n- it's done: the `git push --dry-run origin main` blocked, and don't retry\nEOF\n";
        let commit = "git add a b && git commit -q -m \"feat: x\n\nbody with push guard words\" && git log --oneline -1";
        format!("{python}{prose}{commit}")
    }

    #[test]
    fn heredoc_and_quote_cases() {
        let allow = [
            trial_command(),
            "cat > f <<EOF\ngit push origin main\nEOF".to_string(),
            "cat > f <<-EOF\n\tgit push origin main\n\tEOF\ngit status".to_string(),
            "git commit -m \"it's the push guard\"".to_string(),
            "echo 'it's a push\ngit status".to_string(),
        ];
        for command in &allow {
            assert_eq!(
                decide(command, None),
                Decision::Allow,
                "should allow: {command}"
            );
        }
        let block = [
            "bash <<'EOF'\ngit push origin main\nEOF".to_string(),
            "sh -s <<EOF\ngit push origin main\nEOF".to_string(),
            "git push \"origin main".to_string(),
            "cat <<EOF\nx\nEOF\ngit push origin main".to_string(),
        ];
        for command in &block {
            assert!(is_block(command, None), "should block: {command}");
        }
    }

    const MAIN: Option<&str> = Some("main");
    const FEATURE: Option<&str> = Some("feature");

    /// Stage 5 review gaps: redirections, wrappers, shells, substitutions, abbreviations,
    /// destinations, config rerouting and `cd`.
    #[test]
    fn review_gap_blocks() {
        let cases: &[(&str, Option<&str>)] = &[
            ("git push origin 2>&1", MAIN),
            ("git push origin 2>&1 | tail -5", MAIN),
            ("git push origin > /dev/null", MAIN),
            ("git push origin &>/dev/null", MAIN),
            ("/usr/bin/git push origin main", FEATURE),
            ("timeout 120 git push origin main", FEATURE),
            ("time git push origin main", FEATURE),
            ("nice -n 5 git push origin main", FEATURE),
            ("xargs git push origin main", FEATURE),
            ("env -i git push origin main", FEATURE),
            ("env -i FOO=1 git push origin main", FEATURE),
            ("if ! git push origin main; then echo no; fi", FEATURE),
            ("{ git push origin main; }", FEATURE),
            ("while git push origin main; do :; done", FEATURE),
            ("bash -lc 'git push origin main'", FEATURE),
            ("bash -ec \"git push origin main\"", FEATURE),
            ("/bin/bash -c 'git push origin main'", FEATURE),
            ("zsh -c 'git push origin main'", FEATURE),
            ("eval 'git push origin main'", FEATURE),
            ("eval git push origin main", FEATURE),
            ("echo $(git push origin main)", FEATURE),
            ("echo \"$(git push origin main)\"", FEATURE),
            ("echo `git push origin main`", FEATURE),
            ("cat <<'EOF' | sh\ngit push origin main\nEOF", FEATURE),
            ("cat <<'EOF' | bash\ngit push origin main\nEOF", FEATURE),
            ("cat <<'EOF' | zsh\ngit push origin main\nEOF", FEATURE),
            ("grep x <<< \"$s\"\ngit push origin main", FEATURE),
            ("git push --force-w origin f2", FEATURE),
            ("git push --forc origin f2", FEATURE),
            ("git push --mirr origin", FEATURE),
            ("git push --al origin", FEATURE),
            ("git push --prune origin", FEATURE),
            ("git push --pru origin", FEATURE),
            ("git push origin heads/main", FEATURE),
            ("git push origin x:heads/main", FEATURE),
            ("git push origin x:refs/heads/main", FEATURE),
            ("git push origin @", MAIN),
            ("git push origin HEAD", MAIN),
            ("git push origin @:main", FEATURE),
            ("B=main; git push origin $B", FEATURE),
            ("git -c remote.origin.push=x push origin feature", FEATURE),
            ("git -c Alias.p=push push origin feature", FEATURE),
            ("git -c push.default=x push origin feature", FEATURE),
            ("git -c url.x.insteadOf=y push origin feature", FEATURE),
            ("git config alias.sync push", FEATURE),
            (
                "git config --global alias.sync '!git push origin main'",
                FEATURE,
            ),
            ("git config remote.origin.push main", FEATURE),
            ("git config remote.origin.mirror true", FEATURE),
            ("git config push.default matching", FEATURE),
            ("git config set alias.x push", FEATURE),
            ("cd /elsewhere && git push", FEATURE),
            ("pushd x; git push origin", FEATURE),
        ];
        for (command, branch) in cases {
            assert!(
                is_block(command, *branch),
                "should block: {command} (branch {branch:?})"
            );
        }
    }

    #[test]
    fn review_gap_allows() {
        let cases: &[(&str, Option<&str>)] = &[
            ("git push origin feature 2>&1 | tail -3", FEATURE),
            ("git push origin 2>&1", FEATURE),
            ("timeout 60 git push -u origin feature", FEATURE),
            ("git config --get remote.origin.url", FEATURE),
            ("git config --list", FEATURE),
            ("git config --global --get alias.x", FEATURE),
            ("git config user.name", FEATURE),
            ("git log --oneline origin/main..HEAD", FEATURE),
            ("bash -c 'cargo test'", FEATURE),
            ("cat <<'EOF' > f\nrun git push origin main\nEOF", FEATURE),
            ("cd sub && git push origin feature", FEATURE),
            ("git push origin HEAD", FEATURE),
            ("git push origin @", FEATURE),
            ("echo $(date)", FEATURE),
            ("grep x <<< \"$s\"\ngit status", FEATURE),
        ];
        for (command, branch) in cases {
            assert_eq!(
                decide(command, *branch),
                Decision::Allow,
                "should allow: {command} (branch {branch:?})"
            );
        }
    }

    /// Final hardening round: branch changes, git environment, arithmetic, here-strings, xargs,
    /// config flags, comments, wrappers, `-c` options, per-segment nesting and `gh` merges.
    #[test]
    fn final_round_blocks() {
        let cases: &[(&str, Option<&str>)] = &[
            ("git checkout main && git push", FEATURE),
            ("git switch main; git push origin", FEATURE),
            (
                "git checkout main && git merge feature && git push",
                FEATURE,
            ),
            ("git switch -c main && git push -u origin HEAD", FEATURE),
            ("git rebase main && git push", FEATURE),
            ("git worktree add ../w main && git push", FEATURE),
            ("GIT_DIR=../o/.git git push", FEATURE),
            ("export GIT_DIR=/x/.git; git push", FEATURE),
            ("export GIT_WORK_TREE=/x; git push origin HEAD", FEATURE),
            ("echo $((1<<2))\ngit push origin main", FEATURE),
            ("((x = 1<<2))\ngit push origin main", None),
            ("bash <<< 'git push origin main'", FEATURE),
            ("echo main | xargs git push origin", FEATURE),
            ("echo main | xargs -I{} git push origin {}", FEATURE),
            ("git config --file .git/config alias.p push", FEATURE),
            (
                "git config -f .git/config remote.origin.push feature:main",
                FEATURE,
            ),
            ("git config --blob b alias.p push", FEATURE),
            (
                "# Let's push the branch\ngit push origin main\n# that's it",
                FEATURE,
            ),
            (
                "timeout 60 bash <<'EOF'\ngit push origin main\nEOF",
                FEATURE,
            ),
            ("sudo -u x bash <<EOF\ngit push origin main\nEOF", FEATURE),
            ("env -i bash <<EOF\ngit push origin main\nEOF", FEATURE),
            ("sh -c -- 'git push origin main'", FEATURE),
            ("bash -c -x 'git push origin main'", FEATURE),
            ("caffeinate -t 5 git push origin main", FEATURE),
            ("caffeinate -w 12 git push origin main", FEATURE),
            ("watch -n 5 git push origin main", FEATURE),
            ("builtin cd /x && git push", FEATURE),
            ("cd /x && echo \"$(git push)\"", FEATURE),
            ("cd /x && bash -c 'git push'", FEATURE),
            ("cd /x && bash <<EOF\ngit push\nEOF", FEATURE),
            ("gh pr merge 5 --squash --admin", FEATURE),
            ("gh pr merge --admin 5", FEATURE),
            ("gh api -X PUT repos/o/r/pulls/5/merge", FEATURE),
            ("gh api --method PUT repos/o/r/pulls/5/merge", FEATURE),
            ("gh api --method=PUT repos/o/r/pulls/5/merge", FEATURE),
        ];
        for (command, branch) in cases {
            assert!(
                is_block(command, *branch),
                "should block: {command} (branch {branch:?})"
            );
        }
    }

    #[test]
    fn final_round_allows() {
        let cases: &[(&str, Option<&str>)] = &[
            (
                "git checkout -b feature2 && git push -u origin feature2",
                FEATURE,
            ),
            ("cd /repo && git push -u origin feature", FEATURE),
            (
                "gh pr merge 5 --squash --delete-branch --match-head-commit abc123",
                FEATURE,
            ),
            ("gh api repos/o/r/pulls/5/merge", FEATURE),
            ("echo $((1<<2))", FEATURE),
            ("# comment with git push in it", FEATURE),
            ("echo $#\ngit status", FEATURE),
            ("git config --file .git/config user.name x", FEATURE),
            ("timeout 60 bash <<'EOF'\ngit status\nEOF", FEATURE),
            ("bash -c -x 'cargo test'", FEATURE),
        ];
        for (command, branch) in cases {
            assert_eq!(
                decide(command, *branch),
                Decision::Allow,
                "should allow: {command} (branch {branch:?})"
            );
        }
    }

    #[test]
    fn final_round_messages() {
        let reason = |command: &str, branch| match decide(command, branch) {
            Decision::Block(reason) => reason,
            Decision::Allow => panic!("should block: {command}"),
        };
        let nested = reason("bash -c 'git push origin main'", FEATURE);
        assert_eq!(
            nested.matches("guard-push blocked this command:").count(),
            1,
            "{nested}"
        );
        let unknown = reason("git checkout main && git push -u origin HEAD", FEATURE);
        assert!(
            unknown.contains("cannot tell which branch `HEAD`/a bare push sends"),
            "{unknown}"
        );
        assert!(!unknown.contains("pushes to `main`"), "{unknown}");
        let xargs = reason("echo main | xargs git push origin", FEATURE);
        assert!(
            xargs.contains("arguments are not known until the command runs"),
            "{xargs}"
        );
        let admin = reason("gh pr merge 5 --admin", FEATURE);
        assert!(
            admin.contains("merging without the required checks is not allowed for agents"),
            "{admin}"
        );
    }

    #[test]
    fn run_reads_hook_json() {
        let branch_of = |_: &str| None;
        let push = r#"{"tool_input":{"command":"git push origin main"},"cwd":"."}"#;
        assert!(matches!(
            decide_hook_input(push, branch_of),
            Decision::Block(_)
        ));
        let ok = r#"{"tool_input":{"command":"git status"},"cwd":"."}"#;
        assert_eq!(decide_hook_input(ok, branch_of), Decision::Allow);
        match decide_hook_input("{}", branch_of) {
            Decision::Block(reason) => assert!(reason.contains("tool_input.command"), "{reason}"),
            Decision::Allow => panic!("an empty object must be blocked"),
        }
        assert!(matches!(
            decide_hook_input("not json", branch_of),
            Decision::Block(_)
        ));
    }
}
