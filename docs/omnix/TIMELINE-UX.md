# Timeline UX — what good editors do, and what Omnix should take

The user asked (2026-10-23) for maximum ease and accessibility in the timeline:
"look at best practices of good timeline UIs — simple mobile apps and
Windows/Mac — and let's try them in Omnix". This is the research and the plan.
Built and checked since: packs 1-3 (targets and frame, click routes,
snapping) and 7 (with the sound lanes); the rest wait as `ROADMAP.md`
cards (skimmer, action bar, typed time, overview strip). Each item that
gets built moves to `EDITOR-GAPS-DONE.md`.

Sources: three research passes, 2026-10-23. The first covered mobile apps
(CapCut, iMovie iOS, LumaFusion, VN, InShot, DJI LightCut/Studio, Google
Photos). The second covered desktop apps (Final Cut Pro, Resolve Cut page,
Premiere 25, Rush, Clipchamp, iMovie Mac, Shotcut, Kdenlive, Descript). The
third covered accessibility guidance (WCAG 2.2, Apple HIG, Microsoft, GNOME
HIG, NN/g, Reaper+OSARA). The key URLs are at the bottom. CapCut's own docs
say little about its UI, so the points marked "observed" come from using the
app, not from any source.

## What Omnix already does right (code facts)

- **Magnetic main lane** (user, 2026-10-23: "magnetism is handy after all";
  keep it, a gap only as a blank cut, gap 32). Cuts sit end to end with no
  gaps (`Project.cuts`).
  Moving one reorders the list (`Drag::Move`). Deleting one closes the hole.
  This is FCP's primary storyline and CapCut's main track; good editors make
  it the default.
- **Music anchored to time**, not to clips: deleting a cut does not shift the
  songs. The research recommends exactly this.
- **Split at the playhead**: the C key or the scissors (`edit.rs::split`).
- **Unlimited undo/redo**: Ctrl+Z, Ctrl+Shift+Z or Ctrl+Y (`Edit::undo`).
- **Zoom to fit**: Ctrl+0, plus a zoom slider and wheel zoom around the
  pointer.
- **Volume line on the clip**, with points added by Ctrl+click.
- **Lane height** can be scaled from 0.75× to 3× with the divider
  (`timeline::SCALES`).
- **Cursor changes shape** over edges: `ResizingHorizontally` over an edge,
  `Grab` over a block.
- **Bin buttons**: «в кінець», «у курсор», «на склейку». These are Resolve's
  Append, Insert and Smart Insert, done with no In/Out marks.

## Where Omnix is weak (measured in `src/player/timeline.rs`)

| What | Now | Guidance |
|---|---|---|
| Edge hit zone | ~~`PICK = 7` px each side → 14 px~~ 24 px (`EDGE`, pack 1, 2026-11-02) | ≥ 24 px (WCAG 2.5.8); drawn thin is fine |
| Selection frame | ~~2 px stroke~~ 3 px over a dark 5 px line (pack 1) | thick and distinct from focus; ≥ 3:1 contrast (WCAG 1.4.11) |
| Snapping | music clips only (`snap`, `SNAP = 8`), no visible sign | snap edges, playhead, joins and markers; a line flashes on snap; a key toggles it |
| Feedback while dragging | ~~none~~ the readout by the pointer (pack 1) | live "+0:01.2 · 4.3 s" next to the pointer |
| Drag-only actions | trim, move, retime keyframes | each needs a click or key route as well (WCAG 2.5.7) |
| Hover preview | none on the timeline | skimmer: hover shows the frame in the viewer, silent |
| Undo | keys only | always-visible ↶ ↷ buttons; an «Скасувати» toast after a delete |
| Colour as the only signal | lane colours: orange overlay, teal text, yellow music, cyan sound, purple effects | add an icon or label per lane; check in grayscale |
| Time display | fixed-width time in the transport row, not typeable | large monospace time, click to type «1:32» or «+2s» |
| UI scale | lanes only | text and buttons too (iced `scale_factor`), 100–200 % |

## Patterns worth taking, grouped

### 1. Big, forgiving targets (accessibility, cheap)
- Hit zone ≥ 24 px around every edge and handle, even when it is drawn 2 px
  wide. When a clip is narrower than 3 zones, the pointer's third (left,
  middle or right) decides which part you get.
- Trim handles show only on the selected clip (iMovie, InShot), so a stray
  drag does not trim. The handle is yellow where there is spare footage
  beyond the edge and red at the end of the file (FCP).
- A thick selection frame, plus a separate focus ring for the keyboard.
  Already user ask #7 in `EDITOR-GAPS.md`.

### 2. See what will happen before it happens
- Snapping with a visible line and a key that turns it off while held
  (FCP's N; Shotcut uses Alt). Snap targets: the playhead, joins, markers,
  and later the beats.
- A ghost of the clip and an insert line while you drag (Resolve's smart
  indicator). The bin buttons should show where «на склейку» will land.
- A live number next to the pointer while trimming or moving: the change and
  the new length.

### 3. Every drag has a button or a key (WCAG 2.5.7)
- Trim start or end to the playhead: Alt+[ and Alt+] (FCP), plus buttons in
  the action bar.
- Move the cut one place left or right: buttons, or Alt+← and Alt+→.
- Type a length (FCP's Ctrl+D) and type a time to jump to (Reaper's Ctrl+J).
- Escape during any drag puts everything back.

### 4. Tools follow the selection (CapCut, Premiere 25 Properties)
- One action bar above the timeline. With nothing selected it shows project
  tools: add clip, music, text. With a cut selected: split, trim to playhead,
  delete, volume, speed, view. With a song selected: its own set. In Omnix's
  own style: icon buttons whose hover hint (`tip()`) names the key, so the
  hint teaches the shortcut. Upstream's rule says no help text in the UI;
  whether to add short words beside the icons is for the user to decide.

### 5. Look without clicking
- **Skimmer** (FCP, iMovie): hovering over the timeline shows that frame in
  the viewer while the playhead stays put. Silent by default; S toggles it.
  The bin card "скімер" already wants the same thing for thumbnails, so both
  share one piece of code.
- **Overview strip** (Resolve's dual timeline, done cheaply): when zoomed in,
  a thin strip of the whole edit above the lanes. Clicking it jumps there.
- **Follow the playhead** while playing when zoomed in: Omnix already pages
  the window to keep it in sight.

### 6. Undo you can see
- ↶ ↷ buttons always on screen (CapCut). After a delete, a toast reading
  "Видалено · Скасувати" instead of a "are you sure?" dialog (NN/g).
- Later: a short history list naming each step (Resolve's Edit ▸ History).

### 7. Sound the simple way (ties to gaps 3 and 4)
- Fade handles on the top corners of a sound clip, shown on hover (FCP,
  iMovie).
- **Auto-duck**: music lowers itself under speech. One toggle and one "how
  much" slider. It produces ordinary volume points you can then edit (Rush,
  Premiere). This fits Omnix well: the cleaned voice already tells us where
  the speech is.
- Lanes by role, each with a name and an icon: voice, music, effects.

### 8. Readable time
- Large monospace time. Click it to type «1:32», «+2s» or «-10f».
- An optional "1 хв 32 с" format for family use.

### 9. Scale and calm
- A UI scale setting of 100–200 % covering fonts, buttons and hit zones, plus
  "comfortable" density presets (lanes 48 px and up).
- Lane colours from the colour-blind-safe Okabe-Ito palette, always with a
  name or icon beside them.
- Reduce motion: little animates in Omnix today. Keep it that way, and never
  flash more than 3 times a second.

### 10. Bigger ideas, later
- **Centred playhead** (CapCut, iMovie iOS, and an option in Shotcut and FCP):
  the strip scrolls under a fixed line. Easy to aim at, but desktop users
  expect a moving playhead. Offer it as a setting, not the default.
- **Clips attached to a cut** (FCP connected clips, CapCut track linkage):
  overlays, B-roll and voice-over move with their cut. This is the clean
  form of user ask 4B (B-roll over talk).
- **Text-based editing** (Descript): delete words from the transcript and
  the video is cut to match. footage-agent already has speech, so this is a
  natural later step.
- **Beat markers** from the music, which cuts can snap to (CapCut Auto Beat).
  Ties to the «Гучність і біт» card.
- **Assembled from marks** (DJI LightCut highlights): footage-agent's
  «→ Omnix» already does this.

## Keyboard: conflicts to settle before adding keys

Omnix is a 360 viewer as well, and its keys are taken by looking around:

| Standard key | Standard meaning | Omnix today |
|---|---|---|
| J / K / L | back / stop / forward, faster on each tap | J = keyframe ease, K = add keyframe |
| ↑ / ↓ | previous / next join | look up / down |
| I / O | mark in / out | free |
| E / W / Q / D | append / insert / connect / overwrite | E and Q = roll the view |
| S | skimmer on/off | stabilisation mode |
| N | snapping on/off | free |
| Alt+[ / Alt+] | trim to playhead | free |

**Decided direction (user, 2026-10-23): two modes.** Edit is the default,
and its keys are the standard editing ones. Selecting a 360 clip lights up
a switch to 360 framing, where today's view keys apply. Gap 10 in
`EDITOR-GAPS.md` holds the details to settle. Free keys (N, Alt+[ / Alt+],
Alt+← / Alt+→) can go in before it.

## Proposed order

All of it is one milestone, «Зручний таймлайн», in the user's own order
beside the other work (user, 2026-10-23). The first three packs are small,
need no project format change, and are each checked by the user before the
next starts. The two modes (gap 10) come before any pack that adds
clashing keys (skimmer's S, J/K/L, ↑/↓).

1. ~~**Targets and frame.**~~ Done. 24 px hit zones, thick selection frame (gap 7),
   handles only on the selected clip, live number while dragging.
2. ~~**Click routes.**~~ Done. Trim to playhead, move left/right, ↶ ↷ buttons, the
   undo toast after a delete, Escape cancels a drag.
3. ~~**Snapping.**~~ Done. Snapping for cuts, overlays and the playhead with a visible
   line; N toggles it.
4. **Skimmer** on the timeline and in the bin (one shared piece).
5. **Action bar** that follows the selection.
6. **Time you can type into**. (The UI scale setting is done: 115% by default, A−/A+ in the top bar, `a5dbe0e`.)
7. ~~With sound lanes (gap 3)~~ Done: fade corners, auto-duck, lanes by role.
8. Later: overview strip, attached clips (4B), beat markers, text-based
   editing, centred playhead as an option.

## Sources (main)

- WCAG 2.2: target size https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html ·
  dragging https://www.w3.org/WAI/WCAG22/Understanding/dragging-movements.html ·
  non-text contrast https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html ·
  use of colour https://www.w3.org/WAI/WCAG22/Understanding/use-of-color.html
- Microsoft keyboard accessibility https://learn.microsoft.com/en-us/windows/apps/design/accessibility/keyboard-accessibility
- GNOME HIG accessibility https://developer.gnome.org/hig/guidelines/accessibility.html
- NN/g heuristics https://www.nngroup.com/articles/ten-usability-heuristics/ · Fitts https://www.nngroup.com/articles/fitts-law/
- Reaper keyboard-only editing https://www.reaperaccessibility.com/wiki/Basic_editing
- FCP magnetic timeline https://support.apple.com/guide/final-cut-pro/verb8fcfc133/mac ·
  skimming https://support.apple.com/guide/final-cut-pro/vere9ba3609/mac ·
  snapping https://support.apple.com/guide/final-cut-pro/ver9f7888dc3/mac ·
  clip appearance https://support.apple.com/guide/final-cut-pro/ver4e2edcc/mac ·
  fades https://support.apple.com/guide/final-cut-pro/ver8e3f1f04/mac
- Resolve Cut page (smart insert, dual timeline, auto trim tool) https://www.blackmagicdesign.com/products/davinciresolve/cut
- Clipchamp gaps https://support.microsoft.com/en-us/Clipchamp/how-to-remove-gaps-on-the-timeline
- Premiere auto-ducking https://helpx.adobe.com/premiere-pro/how-to/automatic-audio-ducking.html
- Premiere 25 rounded-corners complaints https://community.adobe.com/t5/premiere-pro-beta-discussions/now-in-beta-rounded-corners-in-the-timeline/m-p/14819833
- iMovie iOS timeline https://support.apple.com/guide/imovie-iphone/knaddae6c843/ios ·
  music https://support.apple.com/guide/imovie-iphone/kna257fc2a9/ios
- LumaFusion timeline https://luma-touch.com/timeline/
- CapCut Auto Cut https://www.capcut.com/help/auto-cut-in-capcut
- Descript https://help.descript.com/hc/en-us/articles/15726742913933
- Okabe-Ito palette https://search.r-project.org/CRAN/refmans/thematic/html/okabe_ito.html
