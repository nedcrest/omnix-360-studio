# The new user guide: plan (gap 109, 2026-11-14)

The guide in `docs/user-guide.md` is rOsmo360's shape with Omnix's words:
about a third still right (projections, stabilisation, keyframes and
ease, follow, LUTs, the aspect lock, export, the command line, the cache),
the window, bars, panels and several keys changed, and nothing on what
Omnix added (sound lanes, groups, sync lock, flat framing, transitions,
automation, cards, voices, mix check, scopes, recording, the bin, markers,
the Dailix bridge). Its 21 pictures are rOsmo360's, all rights reserved:
they go. Dailix has no guide at all. This page is how the new one is
written, what it holds and what gets recorded; the user records the
GIFs and screenshots, Claude writes the text around them.

## How it's written (rules for whoever writes it, Claude included)

From the Google developer documentation style guide, Microsoft's Writing
Style Guide, GitLab's documentation style guide and Diátaxis (sources in
the session of 2026-11-14):

- **Four kinds of page, never mixed** (Diátaxis): a *tutorial* (learn by
  doing, one path start to end), *how-to guides* (one task each, titled by
  the task), *reference* (keys, panels, settings, commands, in tables),
  *explanation* (why trips, videos, voices and sync lock are as they are).
- **Procedures**: a numbered list, one action per step, at most seven
  steps; say where before what («In the **Transitions** panel, select
  **Whip pan**»); start with a verb; UI names in **bold**; menu paths as
  «**Settings** > **Picture**»; input-neutral verbs (select, open, drag,
  enter; not «click»); the keyboard way given too.
- **Pictures illustrate, never replace**: every GIF sits after the steps
  it shows, introduced by a full sentence, never «the image above»; no
  instruction lives only in a picture; no text, code or terminal output
  as an image.
- **Alt text** for every picture: its purpose, ≤ 155 characters, a full
  sentence ending in a period, not «Image of…».
- **One fact, one place**: keys from the shortcuts window
  (`src/player/shortcuts.rs`), not retyped by hand in three pages; check
  every key and label against the code before it's written (two tooltips
  disagree with the code today, see the end).

## Pictures and animations

- **Animated pictures, kept short**: Google and GitLab both advise
  against animated GIFs (size, no pause, accessibility). We use them
  anyway where an action needs motion, with their rules: one action per
  clip, 3–10 s (15 at most), a meaningful still first frame, the steps in
  text beside it, nothing flashing (WCAG 2.3.1). A still screenshot
  wherever one shows it.
- **GIF or MP4**: in the repo, a GIF (a relative path to an `.mp4` doesn't
  play on GitHub). For the README's hero, an MP4 attached through
  GitHub's editor plays as a video, sharper and smaller (10 MB on a free
  plan); a GIF stays as the fallback.
- **Size**: 12–15 fps, 800–1000 px wide, 1–3 MB a GIF (never over 5);
  screenshots PNG, ≤ 1000 px wide, compressed.
- **The same look in every capture**: one window size (1280×800), the
  dark theme, the interface at 100 %, the cursor visible and slow, no
  notifications, the project's lanes filled.
- **What's on screen**: only footage the user is glad to publish (his
  own trips are personal; the test trip is Kos footage of his family:
  his call, shot by shot), no real paths or names of other people, no
  other windows.
- **Files**: `docs/guide/img/<app>-<page>-<action>.gif|png`, lowercase;
  the raw recordings (`.mkv`) kept outside the repo, to render again.

## How to record (KDE Plasma on X11)

- **Simple**: **Peek** (`sudo apt install peek`): a frame over the window,
  record, GIF out. Deprecated since 2023 but the only «draw a box, get a
  GIF» tool that works well on X11 (Spectacle records only on Wayland;
  Kooha goes through the Wayland portal).
- **Best quality**: record lossless, convert after. SimpleScreenRecorder
  (`sudo apt install simplescreenrecorder`) or ffmpeg with `slop` to pick
  the area; then ffmpeg's two-pass palette (or gifski) for the GIF and an
  MP4 beside it:

  ```sh
  sudo apt install slop                       # ffmpeg is installed already
  read W H X Y < <(slop -f "%w %h %x %y")     # drag over the area (or click a window)
  ffmpeg -f x11grab -framerate 30 -draw_mouse 1 -video_size ${W}x${H} -i :1+${X},${Y} \
         -c:v libx264 -preset ultrafast -crf 0 clip.mkv              # q stops
  ffmpeg -i clip.mkv -vf "fps=15,scale=900:-1:flags=lanczos,palettegen=stats_mode=diff" -y palette.png
  ffmpeg -i clip.mkv -i palette.png -lavfi \
    "fps=15,scale=900:-1:flags=lanczos[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle" -y clip.gif
  ffmpeg -i clip.mkv -vf "fps=30,scale=1000:-2" -c:v libx264 -pix_fmt yuv420p -crf 23 -movflags +faststart clip.mp4
  ```

  Too big: lower the width first, then the fps.

## The new guide's shape

`docs/guide/` (Omnix's repo, for both programs; each README links in):

1. **Overview** (explanation): what Omnix and Dailix are, how a trip goes
   from cards and phones to a film; the hero GIF of each.
2. **Install and first run**: build or AppImage; Dailix's welcome (its
   three folders); Omnix's start screen.
3. **Tutorial: your first film**: one trip, start to end: mark a day in
   Dailix (rate, segments, a role), «→ Omnix», trim and order, a song with
   the beats, a title, a transition, export. Short GIFs at each step.
4. **How-to guides**, a task a page (≤ 7 steps, at most one GIF each):
   - Dailix: make a trip; rate and mark clips; clean a voice and choose
     it; open a day in Omnix again.
   - Omnix, editing: trim, split, move, snap; select several and group;
     markers and playing a stretch; sync lock.
   - Omnix, picture: look around a 360 clip and keyframe it; follow a
     subject; frame a phone video (Ken Burns); effects, auto colour and
     scopes; automation lanes; LUTs.
   - Omnix, sound: the four lanes; volume points; J and L cuts; lower the
     music under speech; normalise; beats; record a voice-over; the mix
     check.
   - Omnix, titles and pictures: text, kinds, animations, bars and the
     aspect.
   - Export: one film, a file per cut for another editor, the loudness
     target.
5. **Reference**: the keys (both programs, from their shortcuts windows),
   the panels, the settings, the command lines (Omnix's 25 commands,
   Dailix's), the project files.
6. **Explanation**: trips, videos and the folder layout; what a cut's
   voice is; sync lock and groups; the clip cache.
7. **Troubleshooting**, then the changelog.

## The shot list

S: a still is enough; G: a GIF. **Bold**: the first round, the ones the
READMEs and the tutorial need.

READMEs: **`omnix-hero`** (G 12 s: a finished edit playing, a 360 look
while it plays, a crossfade, the meter); **`dailix-hero`** (G 12 s: Home,
a trip, a day, rate three clips, a segment with a role, «→ Omnix»);
**`omnix-window`** (S: the editor with every area named).

Omnix: `start-cards` (G), `new-menu` (S), `bin-panel` (G), `bin-view-
segments` (G), **`framing-360`** (G), `projections` (G), `stabilisation`
(G), `follow-colour` (G), **`flat-frame`** (G), **`trim-split`** (G),
`move-snap` (G), **`groups`** (G), `markers-range` (G), `crossfade` (G),
**`transitions-panel`** (G), `transitions-lane` (G), `sound-lanes` (S),
**`sound-menu`** (G), `volume-points` (G), `jl-cut` (G), `beats` (G),
`duck-normalise` (G), **`sync-lock`** (G), `record-voice-over` (G),
**`mix-check`** (G), `effects-panel` (S), `auto-colour-scopes` (G),
**`automation-lane`** (G), `text-title` (G), `overlay-kinds` (G),
`bars-aspect` (G), `export-panel` (S), `settings` (S), `shortcuts` (S),
`dailix-voices` (S).

Dailix: `welcome` (S), `home` (S), `new-trip` (S), `trip-page` (G),
**`rating`** (G), **`segments-flags`** (G), `voice-row` (G),
**`to-omnix-question`** (G), `settings-keys` (S).

Each shot's state (which trip, what's selected) is in the session's
feature map; it goes into the page the shot belongs to as the page is
written.

## Found while mapping

- The **Snapping** button's tooltip says Shift drags free; the code and
  the shortcuts window say Alt.
- The **Play range** tooltip says Shift and a drag on the ruler; the
  shortcuts window says on the lanes' empty stretch (both work?).
- Dailix's README lists neither `dailix about` nor `dailix voice …`.
