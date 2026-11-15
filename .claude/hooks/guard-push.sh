#!/bin/sh
# PreToolUse hook for Bash: blocks `git push` to main and force pushes (M3.15, spec 4.6).
# Exits 0 at once unless the hook input mentions `push`; otherwise hands it to
# `cargo xtask guard-push`, which exits 0 (allow) or 2 (block, reason on stderr).
# Claude Code treats any status other than 2 as "allow", so if cargo itself fails
# (xtask does not compile, cargo missing) the guard fails closed: it blocks input
# that names `git` and then `push`, and allows the rest.
input=$(cat)
case $input in
*push*) ;;
*) exit 0 ;;
esac
cd "$(dirname "$0")/../.." || exit 2
printf '%s' "$input" | cargo -q xtask guard-push
status=$?
case $status in
0 | 2) exit "$status" ;;
esac
if printf '%s' "$input" | grep -Eq '(^|[^[:alnum:]_-])git([^[:alnum:]_-].*)?[^[:alnum:]_-]push([^[:alnum:]_-]|$)'; then
  echo "guard-push could not run (cargo exited $status); blocking a command that mentions git push. Fix xtask, or the maintainer can push with ! git push ..." >&2
  exit 2
fi
exit 0
