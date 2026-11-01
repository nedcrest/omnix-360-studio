---
name: roadmap
description: Keep docs/omnix/ROADMAP.md current — the file the user's live roadmap page draws. Use whenever work on Omnix or footage-agent starts, finishes, waits for the user's test, or the user throws in a new idea.
---

# Keeping the roadmap current

The user follows the project on a live page (`docs/omnix/roadmap/`, desktop
shortcut "Omnix — дорожня карта", http://127.0.0.1:8770/). It redraws itself
from `docs/omnix/ROADMAP.md` the moment that file changes. Update it in the
same step as the work, not at the end of a session: the user asked that
nothing be lost after a forced compact.

## When

- A task starts → its card goes to `## Зараз` with `статус: робимо`.
  «Зараз» holds the whole group being worked on, not one card (user,
  2026-11-01): a group of ideas and tasks, each its own card, `робимо`
  until built.
- Code is done and needs the user's eyes or ears → `статус: чекає перевірки`
  (the page shows these in a band at the top).
- The user confirms it works → move the card to `## Зроблено` with `коли:` and
  `коміт:`; drop `статус`.
- The user throws in an idea → a card in `## Ідеї` (or `## Далі` / `## Потім`
  if they gave it an order), and the full idea in `EDITOR-GAPS.md`.
- Order changes → move cards between columns. Keep `## Зараз` short.
- Every new card gets a `віха` (milestone). The milestones and their order
  are the numbered list before `## Зараз` (`1. Name — why`): the page reads
  it for the order of its progress strip. A new milestone goes into that
  list too, and into `EDITOR-GAPS.md`'s "Milestones" section. Regroup
  freely when a better grouping shows (user, 2026-10-23: ideas come
  unsorted; sorting them is our job).

## Format (the page's parser is strict)

```
## Зараз | Далі | Потім | Ідеї | Зроблено      ← exactly these column names

### Card title, Ukrainian, plain words
- статус: робимо | чекає перевірки             ← only in Зараз
- де: Omnix, footage-agent                     ← comma-separated
- віха: Зручний таймлайн                       ← milestone: groups cards; the page shows its progress
- з: YYYY-MM-DD                                ← started (Зараз)
- коли: YYYY-MM-DD                             ← finished (Зроблено)
- коміт: abc1234, def5678
- пункт: 4B                                    ← number in EDITOR-GAPS.md

One or two plain sentences: what it gives the user, not how it's built.
```

- Field lines come right after the `###` heading, before any text, each as
  `- ключ: значення`. Unknown keys are ignored; the ones above are shown.
- Text after a blank line is the card's description, joined into one
  paragraph. No nested lists, no tables.
- Anything before the first `##` is a note for readers, not drawn.
- Card titles are their identity: rename only on purpose (a renamed card
  flashes as new).
- Moving a card by script: it ends at the next `### ` **or** `## `,
  whichever comes first. Cut only to the next card and a column heading
  between goes with it (lost twice, 2026-11-01: «Далі» vanished and its
  cards fell into «Зараз»). Check the four headings after every move.

## Language

Cards are for the user: Ukrainian, short sentences, why before what; an
English term in brackets the first time (`~/.claude/rules/talking-to-dmytro.md`).

## Running the page

`python3 docs/omnix/roadmap/serve.py` (stdlib only); `--install` puts the
shortcut on the desktop and in the menu. A second start just opens the browser.
