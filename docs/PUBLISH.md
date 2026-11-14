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

## Order (the user, 2026-11-14)

Private repos first: put in order there, CI run, then made public. So:
the email rewrite comes **before the first push**, even to a private
repo (whatever is in the history goes public with it later). While
Omnix is private, Dailix's CI can't check it out with its own token: a
read-only token as a secret, or Omnix made public first.

## At publishing (needs the GitHub account)

- Links that are paths today: the README's `../dailix`, Dailix's back to
  Omnix; CI badges.
- `REPO` in `packaging/install.sh` (it downloads from that repo's
  latest release), and the Flatpak metainfo's homepage and screenshot.
- The tag `v0.8.0` and a GitHub release with `dist/`'s archive and
  AppImage (built and checked 2026-11-14; the Flatpak needs
  `org.flatpak.Builder` installed from Flathub, his call).
- `git config user.email` in both repos to the personal address, so new
  commits match the rewritten history.
- The pictures: the README says rOsmo360's screenshots (`docs/
  screenshot.png`, `docs/guide/`) are all rights reserved by their author
  and to be replaced before Omnix is published: his own, or removed.

## The release review (2026-11-14)

The user asked what to close, technically, before the first public
release, from the cards there are. Done already: authorship, the build
from one clone, CI rehearsed, the AI page (a draft he reads), the
project cards (the «outside» closed for now).

The user approved it all (2026-11-14): «should» is a must too; the
working docs cleaned and published; version 0.8.0.

**Must, before the repos go public:**
1. **The small audit** the hook asks for (~4,000 lines since
   2026-11-12: the time seam's guards, the sheet's move, the cards),
   its bugs fixed. Public code is read by strangers first.
2. **No rOsmo360 pictures**: the README's screenshot and the user
   guide's 22 pictures are «all rights reserved» by their author. The
   guide's text is Omnix's already; its pictures are his to replace (or
   the guide ships without them until the GIF guide).
3. **The working docs**: the AI page points at the roadmap, gaps and
   audits; `CONTEXT.md` (both) and the code rules hold personal paths
   and private notes (a passwords file's place in Dailix's). Cleaned and
   published, or kept out — his decision, before the first push.
4. **`CLAUDE.md` at Omnix's root is rOsmo360's** 125 KB maintainer notes
   («paths and footage you won't have»): replaced by Omnix's own short
   one (it points at `AGENTS.md` and the skills), rOsmo360's kept in
   its history.
5. **CI green on GitHub** (the first push, to private repos), and the
   links that are paths today (`../dailix`) made GitHub links.

**Should:**
6. **A version and a tag.** `Cargo.toml` says 0.7.1, rOsmo360's last;
   Omnix's first is its own (0.8.0, or 1.0 if he wants a statement), a
   short changelog of what Omnix added, a GitHub release.
7. **Release builds checked**: `packaging/build-release.sh` (AppImage,
   Flatpak, Windows zip) last ran before the sheet moved in; it should
   build now (no sibling path any more), but untried.
8. **A stranger's first run**: Dailix needs its three places set
   (`dailix places …`); with none it should say so on Home, not just in
   the command line. Omnix starts empty fine.
9. **Dailix's own switch** for the cards' motion (Omnix has one).

**Could:** a tiny demo project made from generated test media, so
someone without a DJI camera sees the editor work in a minute.

**After the release (the «Далі» cards, none needed for it):** the time
seam (paused, consistent: a speed set is refused), copy and paste (102),
outside material (100), video on the overlay lane (4), stingers (25d),
speed (31, 31c). The README's roadmap names them as what's next.

## Rules that stay

Personal footage and paths never go into a published file. Footage is
never committed. Passwords are read where they live, never copied. No
vendor SDKs.
