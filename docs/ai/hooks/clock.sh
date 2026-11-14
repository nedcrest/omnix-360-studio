#!/usr/bin/env bash
# Tells Claude the local time on every message, and how long since the
# previous one: so it stops guessing how many days went by.
# A UserPromptSubmit hook; see README.md beside it.
state="${CLAUDE_PROJECT_DIR:-$PWD}/.claude/hooks/.last-prompt"
now=$(date +%s)
line="Local time: $(date '+%a %Y-%m-%d %H:%M %Z')."
if [ -f "$state" ]; then
  gap=$(( (now - $(cat "$state")) / 60 ))
  if [ "$gap" -ge 120 ]; then line="$line $((gap / 60)) h since the user's previous message.";
  else line="$line $gap min since the user's previous message."; fi
fi
mkdir -p "$(dirname "$state")"
echo "$now" > "$state"
echo "$line"
