#!/bin/sh
# PreToolUse hook for Bash: blocks `git push` to main and force pushes (M3.15, spec 4.6).
# Exits 0 at once unless the hook input mentions `push`; otherwise hands it to
# `cargo xtask guard-push`, which exits 0 (allow) or 2 (block, reason on stderr).
input=$(cat)
case $input in
*push*) ;;
*) exit 0 ;;
esac
cd "$(dirname "$0")/../.." || exit 2
printf '%s' "$input" | cargo -q xtask guard-push
exit $?
