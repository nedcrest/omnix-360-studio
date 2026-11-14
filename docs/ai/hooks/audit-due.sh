#!/usr/bin/env bash
# Counts the code added in Omnix and Dailix since the last audit
# (`last-audit`: one "<repo dir> <commit>" a line) and says so to Claude
# when it is time for a small audit. Silent otherwise.
# After an audit: write the repos' HEADs back into `last-audit`.
root="${CLAUDE_PROJECT_DIR:-$PWD}"
marker="$root/.claude/last-audit"
LINES=2000   # added lines of code (about a day of work)
COMMITS=40
[ -f "$marker" ] || exit 0
added=0; commits=0; since=""
while read -r repo hash; do
  [ -n "$repo" ] || continue
  dir="$root/$repo"
  git -C "$dir" cat-file -e "$hash" 2>/dev/null || continue
  a=$(git -C "$dir" diff --numstat "$hash" HEAD -- src crates 2>/dev/null | awk '{s+=$1} END {print s+0}')
  c=$(git -C "$dir" rev-list --count "$hash"..HEAD)
  added=$((added + a)); commits=$((commits + c))
  [ -n "$since" ] || since=$(git -C "$dir" log -1 --format=%cd --date=format:'%Y-%m-%d %H:%M' "$hash")
done < "$marker"
if [ "$added" -ge "$LINES" ] || [ "$commits" -ge "$COMMITS" ]; then
  echo "Audit due: $added lines of code added and $commits commits in Omnix and Dailix since the last audit ($since). Remind the user and offer the small audit (the code-rules skill, «Audits: small and often»)."
fi
