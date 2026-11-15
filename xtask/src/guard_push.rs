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

/// Decides whether `command` may run. `current_branch` is the checked-out branch of the hook's
/// working directory, when known; a push with no explicit refspec pushes it.
pub(crate) fn decide(command: &str, current_branch: Option<&str>) -> Decision {
    let (stripped, shell_bodies) = strip_heredocs(command);
    // A heredoc that feeds a shell is a script: decide on it as a nested command.
    for body in &shell_bodies {
        if let Decision::Block(reason) = decide(body, current_branch) {
            return Decision::Block(reason);
        }
    }
    let segments = match split_segments(&stripped) {
        Ok(segments) => segments,
        // Fail closed only when the text looks like a push; a command whose quoting the simple
        // parser cannot follow, and that never says `git ... push`, is not the guard's business.
        Err(why) if mentions_git_push(&stripped) => return Decision::Block(blocked(&why)),
        Err(_) => return Decision::Allow,
    };
    for words in &segments {
        if let Some(reason) = check_segment(words, current_branch) {
            return Decision::Block(blocked(&reason));
        }
    }
    Decision::Allow
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

/// True when the command text before a heredoc operator runs a shell (`bash`, `sh -s`, ...).
fn feeds_shell(before: &str) -> bool {
    let line = before
        .rsplit(['\n', ';', '|', '&', '('])
        .next()
        .unwrap_or_default();
    let mut words = line.split_whitespace().filter(|w| {
        (!w.contains('=') || w.starts_with('-'))
            && !matches!(*w, "env" | "command" | "exec" | "sudo" | "nohup")
    });
    words
        .next()
        .is_some_and(|w| matches!(w.rsplit('/').next(), Some("sh" | "bash" | "zsh")))
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

/// Removes heredoc operators and bodies (`<<WORD`, `<<'WORD'`, `<<"WORD"`, `<<-WORD`) from
/// `command`; they are data. Returns the rest, and the bodies of heredocs that feed a shell, which
/// are commands. Quotes are tracked so a `<<` inside a string is not an operator.
fn strip_heredocs(command: &str) -> (String, Vec<String>) {
    let chars: Vec<char> = command.chars().collect();
    let mut out = String::new();
    let mut shell_bodies = Vec::new();
    // Heredocs opened on the current line: (delimiter, tabs allowed, feeds a shell).
    let mut pending: Vec<(String, bool, bool)> = Vec::new();
    let mut quote: Option<char> = None;
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
                '\'' | '"' => quote = Some(c),
                '\\' => {
                    out.push(c);
                    out.extend(chars.get(i + 1));
                    i += 2;
                    continue;
                }
                '<' if chars.get(i + 1) == Some(&'<') && chars.get(i + 2) != Some(&'<') => {
                    let mut j = i + 2;
                    let dash = chars.get(j) == Some(&'-');
                    j += usize::from(dash);
                    while matches!(chars.get(j), Some(' ' | '\t')) {
                        j += 1;
                    }
                    let (word, end) = heredoc_word(&chars, j);
                    if !word.is_empty() {
                        pending.push((word, dash, feeds_shell(&out)));
                        i = end;
                        continue;
                    }
                }
                '\n' if !pending.is_empty() => {
                    out.push('\n');
                    i += 1;
                    for (word, dash, shell) in std::mem::take(&mut pending) {
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

/// Splits a shell command into segments (on `&&`, `||`, `;`, `|`, `&`, newlines and parentheses
/// outside quotes) and each segment into words, with single and double quotes and backslash
/// escapes respected. An unterminated quote is an error.
fn split_segments(command: &str) -> Result<Vec<Vec<String>>, String> {
    let mut segments = Vec::new();
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut chars = command.chars();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(q) => word.push(q),
                        None => return Err("the command has an unterminated quote".to_string()),
                    }
                }
            }
            '"' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => word.extend(chars.next()),
                        Some(q) => word.push(q),
                        None => return Err("the command has an unterminated quote".to_string()),
                    }
                }
            }
            '\\' => {
                in_word = true;
                word.extend(chars.next());
            }
            ';' | '|' | '&' | '\n' | '(' | ')' => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
                if !words.is_empty() {
                    segments.push(std::mem::take(&mut words));
                }
            }
            c if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            c => {
                in_word = true;
                word.push(c);
            }
        }
    }
    if in_word {
        words.push(word);
    }
    if !words.is_empty() {
        segments.push(words);
    }
    Ok(segments)
}

/// Returns why a segment must be blocked, or `None` when it is not a risky push.
fn check_segment(words: &[String], current_branch: Option<&str>) -> Option<String> {
    let mut i = 0;
    // Skip assignments and wrappers that run the next word: `FOO=1 git push`, `sudo git push`.
    while let Some(w) = words.get(i) {
        let is_assignment = w.contains('=') && !w.starts_with('-');
        if is_assignment || matches!(w.as_str(), "env" | "command" | "exec" | "sudo" | "nohup") {
            i += 1;
        } else {
            break;
        }
    }
    let rest = words.get(i..)?;
    let program = rest.first()?;
    // `sh -c "git push origin main"`: look inside the script.
    if matches!(program.as_str(), "sh" | "bash" | "zsh") {
        let script = rest.iter().skip(1).skip_while(|w| *w != "-c").nth(1)?;
        return match decide(script, current_branch) {
            Decision::Block(reason) => Some(reason),
            Decision::Allow => None,
        };
    }
    if program != "git" {
        return None;
    }
    let mut j = 1;
    let mut redirected = false;
    while let Some(w) = rest.get(j) {
        match w.as_str() {
            "-C" | "-c" | "--git-dir" | "--work-tree" | "--namespace" => {
                redirected |= w != "-c";
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
    if rest.get(j).map(String::as_str) != Some("push") {
        return None;
    }
    check_push(
        &rest[j + 1..],
        if redirected { None } else { current_branch },
    )
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
            "--all" | "--mirror" => return Some(format!("`git push {arg}` pushes every branch")),
            "--force" => return Some("force pushes are not allowed".to_string()),
            "-o" | "--push-option" | "--repo" | "--receive-pack" | "--exec" => skip_next = true,
            long if long.starts_with("--force-with-lease") => {
                return Some("force pushes are not allowed".to_string());
            }
            long if long.starts_with("--") => {}
            short if short.starts_with('-') && short.len() > 1 => {
                if short.contains('f') {
                    return Some("force pushes are not allowed".to_string());
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
            None => Some("cannot tell which branch this pushes".to_string()),
        };
    }
    for spec in refspecs {
        if spec.starts_with('+') {
            return Some(format!("`{spec}` is a force push"));
        }
        let destination = spec.rsplit(':').next().unwrap_or(spec);
        let destination = destination
            .strip_prefix("refs/heads/")
            .unwrap_or(destination);
        let destination = if destination == "HEAD" {
            current_branch.unwrap_or("main")
        } else {
            destination
        };
        if destination == "main" {
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
