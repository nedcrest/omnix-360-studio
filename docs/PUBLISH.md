# Publishing Omnix and Dailix (gap 109, 2026-11-14)

The user is putting both programs on GitHub as his AI-driven / AI-assisted
private project, to show what they do and how they were built. This comes
before the time seam (`docs/omnix/TIME-SEAM.md`, paused after its step 1,
everything green) and the features. **Nothing is pushed until the user
says so.**

## Where things stand (scan of 2026-11-14)

| | Omnix | Dailix |
|---|---|---|
| Commits | 1119 (175 rOsmo360's, 942 his) | 113 |
| Commits co-authored by Claude | 1105 | 113 |
| His work | 2026-10-22 .. 2026-11-12 | 2026-11-04 .. 2026-11-12 |
| Rust | ~47,800 lines, 350 tests (+13 CLI tests on real ffmpeg) | ~6,200 lines, 50 tests |
| Licence | MIT or Apache-2.0 (rOsmo360's) | MIT or Apache-2.0 |
| Remote | rOsmo360 as `upstream` only | none |

Clean:
- No footage, frames or sample pictures in either history: the 28 images
  ever committed are rOsmo360's (its user guide, icons, screenshot).
- No secrets, tokens or personal paths in code or tests; rOsmo360 is
  credited in the README and in the app's footer.

To fix:
- **Author email**: all his commits carry his former work address.
- **Pictures**: the README's screenshot and `docs/guide/` are rOsmo360's,
  showing its old interface.
- **Building**: Dailix finds Omnix at `../omnix-360-studio` (a path
  dependency), so a clone of Dailix alone doesn't build.
- **No CI.**
- **No page on how it was made with AI**, the point of publishing.
- **Working docs** (CONTEXT, ROADMAP, AUDIT, EDITOR-GAPS, `CLAUDE.md`,
  `.claude/` skills) hold personal paths and private notes; whether they
  go public is decided later (the user, 2026-11-14).

## Decisions (the user, 2026-11-14)

- Publication first; the seam waits.
- Two repos. Omnix keeps rOsmo360's history and credits; Dailix takes
  Omnix as a git dependency and links back to it.
- The author email becomes a personal one, rewritten in both histories.
- The working docs: decided later.

## Steps (one local commit each, in each repo)

1. **Author email.** Back up both repos (`git bundle create … --all`),
   then rewrite only his commits (`--author=Halushko`) to the personal
   address, keeping names, dates and `Co-Authored-By` lines; rOsmo360's
   commits untouched. Check: counts per author before and after, `git
   fsck`, the tests at the new HEAD. Needs: the address, confirmed.
2. **Dailix builds alone.** Omnix as a git dependency at the published
   URL (needs the GitHub account name); working side by side stays one
   line of local Cargo config, outside the repo.
3. **CI** for both: build, tests, clippy on Ubuntu, with the system
   libraries the build needs and ffmpeg for the CLI tests; GPU tests
   skipped where there's no GPU.
4. **READMEs.** What each is and why, one picture or GIF at the top, the
   two programs' link, building, credits. Pictures from the user (his
   own footage, chosen by him) or made from neutral test media.
5. **How it was made with AI** (`docs/AI-DRIVEN.md`, English): the loop
   (he decides and tests, Claude plans, writes, tests, audits), the rules
   (`CLAUDE.md`, `AGENTS.md`, `.claude/skills`), the hooks (the clock, the
   audit tracker), the live roadmap page, the audits, the snapshot guards,
   the numbers; honest about what he did and what Claude did.
6. **Later**: the user guide redone around GIFs he records; the working
   docs (cleaned or kept home); release builds.

## Rules that stay

Personal footage and paths never go into a published file. Footage is
never committed. Passwords are read where they live, never copied. No
vendor SDKs.
