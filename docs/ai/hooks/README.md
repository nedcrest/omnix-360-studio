# The hooks this work runs with

Two Claude Code hooks, run before each of the user's messages
(`UserPromptSubmit`). They live in the folder holding both repos side by
side (`omnix-360-studio/` and `dailix/`), as that folder's
`.claude/hooks/`, and are wired in its `.claude/settings.local.json`:

```json
{
  "hooks": {
    "UserPromptSubmit": [
      { "hooks": [
        { "type": "command", "command": "$CLAUDE_PROJECT_DIR/.claude/hooks/clock.sh", "timeout": 5 },
        { "type": "command", "command": "$CLAUDE_PROJECT_DIR/.claude/hooks/audit-due.sh", "timeout": 15 }
      ] }
    ]
  }
}
```

- **`clock.sh`** gives Claude the local time and how long since the last
  message. Without it, Claude guessed: it spoke of «several days of
  work» since an audit when one had passed (the user, 2026-11-11).
- **`audit-due.sh`** counts the code added to both repos since the last
  audit (`.claude/last-audit`: a line `<repo dir> <commit>` per repo) and
  says when a small audit is due: past about 2,000 lines or 40 commits.
  After an audit, the repos' HEADs are written back into the marker. The
  rule it serves: `.claude/skills/code-rules/SKILL.md`, «Audits: small
  and often».

Why they exist: [`../../AI-DRIVEN.md`](../../AI-DRIVEN.md).
