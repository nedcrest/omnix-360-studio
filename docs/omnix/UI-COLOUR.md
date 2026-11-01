# UI colour: what editing apps do, and a switchable palette for Omnix

Gap 43 (user, 2026-11-01): "analyse best practices for editing software,
look at colour schemes, theoretically add the ability to change the UI's
colour palette". Research done 2026-11-01. Nothing built yet; the proposal
at the end is ready to become a card.

## What editing apps do

**Neutral, dark, nearly colourless chrome.** Two reasons:
- **Judging the picture.** Colours around an image change how its colours
  are seen (simultaneous contrast). A tinted UI beside the viewer pushes
  the eye towards the opposite hue.
- **Comfort.** Editing rooms are dim; a dark UI doesn't dazzle.

Examples:
- **DaVinci Resolve.** Its default UI is slightly blue-grey. A preference,
  «Use gray background for user interface», makes it fully neutral; the
  manual's reason is that the blue tint biases a colourist in a dark
  suite. A second setting greys the viewer's own surround; newer versions
  offer checkered / black / gray behind the image
  ([Resolve 18.6 manual](https://www.steakunderwater.com/VFXPedia/__man/Resolve18-6/DaVinciResolve18_Manual_files/part133.htm),
  [BMD forum](https://forum.blackmagicdesign.com/viewtopic.php?p=1147351)).
- **Premiere Pro.** Only a brightness slider; no colour themes, a frequent
  complaint ([Adobe](https://helpx.adobe.com/premiere-pro-next/learn-the-basics/configure-preferences/appearance-preferences.html)).
- **Kdenlive.** Takes KDE colour schemes; long-standing bugs with dark
  schemes (icons vanishing) ([KDE 439052](https://bugs.kde.org/show_bug.cgi?id=439052)).

**"18 % grey" is about the room, not the app.** A mid grey of 18 %
luminance is about `#777777`. Real editing chrome is far darker,
`#1F1F1F`–`#3A3A3A` (1.4–4 % luminance). No vendor publishes its UI greys;
numbers online are eyedropper guesses. Material Design's darkest surface is
`#121212`, not black, with raised surfaces lighter
([Material](https://m2.material.io/design/color/dark-theme)).

**Colour by kind, then by clip.**
- Final Cut Pro colours clips by **role** (Video, Titles, Dialogue, Music,
  Effects, custom): changing a role's colour recolours every clip with it,
  and audio roles can be shown as their own lanes
  ([Apple](https://support.apple.com/guide/final-cut-pro/verbd20e0664/mac)).
  This is what Omnix's lanes already are.
- Resolve adds **clip colours** (~16 named) from the right-click menu,
  overriding the track's; organisation only, never the output
  ([manual](https://www.steakunderwater.com/VFXPedia/__man/Resolve18-6/DaVinciResolve18_Manual_files/part1013.htm)).
- Reaper: a theme file for the chrome plus per-track colours, saved
  colour slots, an option to tint a track's panel with its colour
  ([Cockos](https://www.cockos.com/wiki/index.php/Preferences_Color_Themes),
  [SWS](https://sws-extension.org/color.php)). FL Studio and Ableton the
  same in outline (not checked against sources).

**Playhead, selection, meters** (common practice, not sourced): a thin,
high-contrast playhead (red, white or blue: the contrast matters more
than the hue); selection as a bright or white outline plus a lighter body,
not a change of hue; meters green / yellow / red, red meaning over full
scale; waveforms a lighter or darker shade of the clip's own colour.

## Accessibility

WCAG 2.x:
- text at least **4.5:1** against its background (3:1 large text), 1.4.3;
- lane fills, lines, handles at least **3:1** against what's next to
  them, 1.4.11;
- **colour never the only cue**, 1.4.1: a shape, a label, a pattern or a
  position too.

For colour-blind users this means lanes should differ in **lightness**,
not only hue, and keep their labels and shapes. Omnix already names every
lane at its left and draws key marks by ease shape; the over-full-scale
bars need a shape of their own (below).

Colour-blind-safe categorical palettes:
- **Okabe–Ito**: `#E69F00` orange, `#56B4E9` sky blue, `#009E73` bluish
  green, `#F0E442` yellow, `#0072B2` blue, `#D55E00` vermillion,
  `#CC79A7` reddish purple
  ([reference](https://search.r-project.org/CRAN/refmans/see/html/okabeito_colors.html)).
- **Paul Tol «bright»**: `#4477AA`, `#66CCEE`, `#228833`, `#CCBB44`,
  `#EE6677`, `#AA3377`, `#BBBBBB`
  ([Tol](https://sronpersonalpages.nl/~pault/data/colourschemes.pdf)).

Both were made for white backgrounds. On a `#1F1F1F` background their dark
members fall short: Okabe–Ito blue 3.2:1, Tol purple 2.7:1 (fails). Lighten
them for a dark UI.

## Around the picture (grading)

- **ITU-R BT.2035** (SDR reference viewing): room about 10 lux, a neutral
  D65 surround at about 10 % of reference white, ~10 cd/m²
  ([ITU](https://www.itu.int/dms_pubrec/itu-r/rec/bt/R-REC-BT.2035-0-201307-I!!PDF-E.pdf)).
- **ITU-R BT.2166 (2025)** says SDR suites now usually follow BT.2100: a
  neutral grey surround at **5 cd/m²**
  ([ITU](https://www.itu.int/dms_pubrec/itu-r/rec/bt/R-REC-BT.2166-0-202502-I!!PDF-E.pdf)).
  The two disagree; both say *neutral*.

For a home editor that means: no colour in the UI right around the viewer;
the viewer's surround a neutral dark grey, black as an option; saturated
lane colours down in the timeline, away from the picture.

## Omnix today (checked in the code)

| Thing | Colour now | Problem |
|---|---|---|
| Theme | iced's Kanagawa Dragon (`#181616` background) | slightly warm, not neutral; fine for cutting, not for judging colour |
| Music lane | the theme's `success` colour (`timeline.rs`, `draw_sound_lane`) | changes hue with every theme; in Kanagawa Dragon a dull `#8a9a7b` |
| SFX / Voice | fixed pink `SFX_COLOUR` / blue `VOICE_COLOUR` | — |
| Clip sound lane and a clip's own volume line | both cyan (`SOUND_LINE`) | the line sits on its own colour: low contrast |
| Text lane / Clip sound | teal / cyan | close hues |
| Overlay lane / over-full-scale bars | both orange (`OVER_COLOUR`) | one colour, two meanings |
| Effects marks | purple `EFFECT_MARK` | — |

Colours are constants scattered through `timeline.rs` and the panels; iced's
`Palette` has no place for lane colours (`background, text, primary,
success, warning, danger` only).

## Proposal

**1. Semantic tokens, not literal colours.** One struct the whole UI
reads: `bg`, `surface`, `surface_raised`, `border`, `lane_bg`, `text`,
`text_muted`, `accent`, `selection`, `playhead`, `warning`, `over`,
`viewer_surround`, and one colour per lane kind (`music`, `sfx`, `voice`,
`clip_sound`, `overlay`, `text_lane`, `effects`, `track_line`, `own_line`).
Lane colours belong to Omnix, not to iced's theme, so switching the theme
no longer recolours Music.

**2. Default «Omnix Neutral» (dark).** Contrast against `#1F1F1F`
(calculated, WCAG):

| Token | Hex | Contrast |
|---|---|---|
| bg | `#1F1F1F` | — |
| surface | `#2A2A2A` | |
| surface_raised | `#353535` | |
| lane_bg | `#242424` | |
| border | `#3D3D3D` | |
| viewer_surround | `#262626` (or black) | |
| text | `#E0E0E0` | 12.5:1 |
| text_muted | `#A6A6A6` | 6.8:1 |
| accent | `#2F6FD6` | white on it 4.8:1 |
| selection | `#FFFFFF` outline, 2 px | |
| playhead | `#FF5A4F` | 5.4:1 |
| warning | `#F2C94C` | |
| over (full scale) | `#FF4D4D`, and a capped bar shape | red as on meters; frees orange |
| Music | `#6FBF73` | 7.4:1 |
| SFX | `#E07FB8` | 6.2:1 |
| Voice | `#5B9BE6` | 5.7:1 |
| Clip sound | `#4FC1C9` | 7.7:1 |
| Track volume line | `#F2C94C` | 10.4:1 |
| Own volume line | `#F2F2F2`, dashed | no longer cyan on cyan |
| Overlay | `#E8954A` | 6.9:1 |
| Text lane | `#9AA7FF` | periwinkle, away from cyan |
| Effects marks | `#B48CE8` | 6.2:1 |

Waveforms: the lane colour at about 80 % over a darker shade (~25 %) of
the same hue.

**3. A colour-blind-safe preset, «Okabe–Ito (dark)»**: Music `#009E73`,
SFX `#CC79A7`, Voice `#56B4E9`, Clip sound neutral `#BBBBBB`, Overlay
`#E69F00`, Text `#F0E442`, Effects `#D55E00`; volume lines white, solid
for the track and dashed for a clip's own; over full scale red with the
capped shape.

**4. Switching it, in Settings → Appearance:**
1. The UI theme picker, as now, plus «Omnix Neutral» made with
   `Theme::custom_with_fn` (exact control of the surface greys; iced 0.14
   derives the weaker and stronger greys in OKLCH and picks readable text
   by itself).
2. A lane palette: Omnix default / Okabe–Ito / Follow theme.
3. Per-lane override: a swatch and a reset for each kind; a warning beside
   it if it falls under 3:1 against `lane_bg`.
4. «Neutral viewer surround» on/off.

Kept in `settings.json` as `{ ui_theme, lane_palette, lane_colours: {
music: "#…", … }, neutral_surround }`, all optional, so old settings load.
A light theme gets darker, more saturated lane variants, chosen by
`extended_palette().is_dark`.

Later: per-clip colour labels, Resolve style.

**5. Fix whatever palette is chosen** (small, can come first): Music off
the theme's `success`; the own volume line no longer cyan on cyan;
over-full-scale red with a shape instead of orange; the text lane away
from cyan.

## Uncertain

Vendor UI greys (none published), Final Cut's default role colours, Tol
«muted» hex values, the surround level (10 cd/m² in BT.2035, 5 in
BT.2166). The contrast ratios are calculated here, not quoted.
