# AI auto-edit — the brief, the rules, the tools

User, 2026-10-23: "the main thing I'd like: an API, a skill and a snippet so
you or another AI can connect and auto-edit, with the rules described
properly. I make the project in the contact sheet with descriptions of what
and where, with marks, and for the AI that is the guideline to build a good
sequence and edit it automatically. Or is that crooked and unrealistic?"

**Status: designed, deliberately not started.** The user, 2026-10-23:
"I wouldn't do AI functions yet: Omnix doesn't have its full working
functionality." It is milestone 9, the last. Speech transcription and faces,
which it will use, are milestone 4 (gaps 29, 30).

## Is it realistic?

Yes for a **good first edit** that the user then polishes. No for a finished
film with no human, because an AI reading frames and metadata can't reliably
judge a joke's timing or a face's emotion. Transcripts close part of that gap.

Most pieces exist already:

- **The brief exists.** The Kos contact sheet (`report/notes.json`) holds
  126 clips, 72 of them with notes saying what happens and what to do with
  it. It also holds 77 marks with roles: `broll` 47, `action` 10, `cut` 7,
  `talk` 6, `reveal` 5, `detail` 2, and 6 flags. A note per day carries the
  story and ideas, e.g. "a cut-up with a voice-over from the meme
  'Де сметана?'". The roles come with meanings in `report.html`:
  - `talk`: someone speaks into the camera;
  - `establish`: opens a scene;
  - `broll`: under music or voice-over;
  - `detail`: an insert;
  - `cut`: a cutaway that covers a cut in talk;
  - `action`;
  - `reveal`;
  - `reaction`.
- **The output format is public and checked.** The project JSON loads with
  `deny_unknown_fields`, and footage-agent's `export/omnix.py` already
  writes it correctly from marks, with the chosen cleaned voices.
- **There are tools for an AI editor.** Upstream built `check` (likely
  mistakes, a line each), `preview` (a contact sheet of a project, a row per
  cut, with the music waveform), `scan` and `info` for "claudeVideoEdit", an
  AI editor. An AI can't watch video, but it can read `preview`'s image and
  `check`'s lines.

## The design: four parts

### 1. The brief (footage-agent writes it)

`04_project/brief-<day>.json`, written beside «→ Omnix». It is everything an
editor needs, in one file, and nothing it has to dig for:

- **The day:** its story note, target length, aspect, style words, music
  (files from `02_audio/music` or none).
- **Clips:** rating, note, length, shot facts from the analysis, the
  cleaned voice if chosen.
- **Marks:** time or span, role, note, flag.
- **What's available:** memes, stingers, sfx and graphics from the bin's
  folders.
- **Later:** transcripts with word times (gap 29) and who is in each clip
  (gap 30).

The contact sheet gains a few brief fields, per day:
- target length;
- music;
- style («спокійно», «динамічно», «смішно»).

Per clip or mark:
- «обов'язково» (must use);
- «не брати» (never use).

### 2. The rules (a skill)

`omnix-automontage` (in `.claude/skills/` here, readable by any agent as
plain Markdown). The rules come in three kinds.

**Story:**
- Chronological by default.
- Each scene opens with `establish`, then `action` or `talk`, ends on a
  `reveal` or `reaction`.
- The day's note wins over the defaults.

**Cutting grammar:**
- Lengths per role (B-roll 2–5 s, talk by whole phrases, never mid-word,
  which needs transcripts or the voice's silences).
- `cut` marks cover jumps in talk.
- No repeated footage (`check repeat`).
- Music under B-roll, ducked under talk.
- Transitions rarely, stingers where a mark says «перебивка».
- No fast pans (`check fast pan`).

**Hard rules:**
- Only fields Omnix knows.
- Write a new versioned file (`omnix-<day>-ai-v1.json`), never over the
  user's.
- Never touch originals.
- Every decision traceable to a mark or a note, listed in a report beside
  the project.

### 3. The tools ("API")

Files and the CLI first, because agents work well with shell and files, and
nothing has to run in the background. Each tool is listed with where it
exists today:

- **Exists now:**
  - `rosmo360 check proj.json`
  - `rosmo360 preview proj.json`
  - `rosmo360 info`
  - `rosmo360 scan`
  - `export/omnix.py` (Python)
- **To add in Omnix:**
  - `rosmo360 schema` (the project's JSON Schema, so an agent can validate
    before saving);
  - `rosmo360 describe proj.json` (a short text summary per cut: edit time,
    clip, in/out, note, sound, effects; cheaper for an AI than the image);
  - the player reloading the project when its file changes, so the user
    watches the AI's version appear.
- **To add in footage-agent:** a small tested Python library (the
  "snippet"): load the brief, pick, order, trim, place music and voice, and
  write through `export/omnix.py`. Agents call it instead of hand-writing
  JSON, so format mistakes can't happen.
- **Later, if needed:** a local HTTP API on the player (open, seek, show a
  cut) so an agent can point the user at things live. Not needed for a
  first version.

### 4. The loop

1. AI reads the brief and the skill.
2. It writes v1 with the snippet, runs `check` and fixes what it finds.
3. It runs `preview`, looks at the sheet against the rules, and writes v2.
4. It writes a short report: what it chose and why, and what it wasn't sure
   about.
5. The user opens v2 in Omnix, watches it, and leaves comments: markers
   with notes (gap 5) or notes in the contact sheet.
6. The AI reads the comments and writes v3. Repeat.

## Steps (gap 28), smallest useful first

- **28a. Brief export** from the contact sheet. footage-agent, small.
- **28b. Skill v0:** the rules above, the snippet, the loop, using only what
  exists today (cuts, notes, cleaned voice, music, crossfades, text). Tried
  at once on Kos 22.08 against the user's own cut. Small to medium.
- **28c. Omnix:** `schema`, `describe`, and reloading the project when its
  file changes. Small to medium.
- **28d. Brief fields in the contact sheet:** target length, music, style,
  must and never. Small.
- **28e. Transcripts in the brief:** Whisper on the cleaned voices, word
  times, so talk is cut on phrases. Shared with subtitles (16). Medium.
- **28f. The review loop:** the user's comments back to the AI (markers with
  notes, gap 5). Medium.

It grows with the other milestones:
- sound lanes and auto-duck give it voice-over and ducking;
- transitions and stingers give it «перебивки»;
- beats let it cut to the music;
- B-roll on the overlay lane gives it cutaways over talk.

## What could go wrong

- **Taste.** The first versions will be mechanical: correct, but flat. The
  rules get better from the user's comments, so record each correction as a
  rule in the skill.
- **Talk cut mid-word** until transcripts or silence detection exist (28e).
- **The format drifting** between Omnix and footage-agent. One schema
  (28c), and `export/omnix.py`'s tests run against it.
