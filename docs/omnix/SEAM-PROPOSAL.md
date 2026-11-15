# The lens seam: what we could offer rOsmo360's author

Written 2026-10-23 at the user's request: a plain account of which of
OpenOSV's ideas are new to rOsmo360. Details and sources: `OPENOSV.md`.
What rOsmo360 already does: its `CLAUDE.md`, the «Seam» section.
**Nothing has been sent.** Whether to offer it is the user's call.
(Written in Ukrainian at first, the user's language; in English since
2026-11-15.)

## What the seam is

The Osmo 360 films through two fisheye lenses, each seeing a little more
than half the world. The program stitches them into a sphere. Where one
picture turns into the other is the **seam**, and every disagreement
between the lenses shows there:
- **Near things double.** A hand on the stick, a cap: each lens sees them
  from its own angle (parallax).
- **A band shows.** One lens is darker or another colour, because the sun
  flares into it and not the other.
- **The ground slides.** The lenses are slightly tilted against each other.

## What rOsmo360 already does

- It takes DJI's factory calibration, as DJI Studio does.
- Every frame, it shifts the lenses towards each other so the details on
  the seam meet (alignment).
- Where the lenses still disagree (a hand, a cap) it doesn't blend them
  but takes one, cleanly: the hand stays solid, not see-through.
- It fades each lens's edge over 2°, so there's no step by the mask.

**Tried and dropped** (we offer nothing new here):
- the best seam line;
- evening out the lenses' brightness (it made things worse, so it's off);
- fitting the flare;
- blending in two bands;
- a wider search.

OpenOSV tried the same and came to the same conclusions.

**What's left unsolved for him:**
1. **A cap's brim by the stick is cut off** (clip 0025, 19.5 s). The cause:
   DJI's masks, which hide the mount, cut away too much there. Of the
   lenses' overlap 4–6° is left instead of the usual 17–19°, and the brim
   «jumps» between the lenses by about 6°. Its tip falls where only one
   lens has the picture, and that one sees rocks there. The author noted
   «masks tighter than DJI's where the hump is wider than the stick» as
   **unexplored**.
2. **A flare band** (clip 0032): where the sun flares one lens, the edge
   shows. Four attempts didn't help.

## What we offer (that he doesn't have)

### 1. Release the mask where both lenses see the same — the main one

**What it does.** Once a clip, take 9 frames and look at the band along
the mount's mask, without the mask. Where both lenses see the same thing
(a match of 0.8 or more) the mask comes off: the mount isn't really
there. Where they see different things the mask stays, with a margin.

**Why it's new.** It's the very idea the author noted as unexplored, with
a way to do it already worked out: OpenOSV does it («Hide Mount: Auto»).

**What it gives.** By the stick, the overlap where both lenses see grows.
The cap's brim can then fall where both see it, and taking one lens
«where they disagree» keeps it solid.

**Careful.** OpenOSV tuned it on mounts on a car, not a stick in a hand.
Whether it's enough for the cap has to be measured.

### 2. Read from the file that it was filmed through a lens guard

**What it does.** The camera writes the lens mode into the file: bare,
with protective glass (ND filters are marked the same), underwater
(`StreamMeta.7 extri_lens_mode`). The glass changes slightly the angle
at which the lens sees the world: +0.5° at 30°, +1.3° at 90°. DJI Studio
takes it into account, so does OpenOSV; rOsmo360 doesn't.

**What it gives.** Clips filmed through a guard or an ND filter stitch
without a shift on the seam. Nothing changes for other clips.

### 3. Check whether the lenses are tilted

**What it does.** The lenses can be turned slightly against each other:
by a knock, heat, a filter. OpenOSV found 0.36° on its clip. rOsmo360's
author checked only the turn about the lens's axis (there's none), not
the tilts.

**What we offer.** First only a **check**: rOsmo360 already measures,
every frame, how far it shifts the lenses on the seam. If, averaged over
many frames, that shift runs round the circle like a wave, the lenses are
tilted. Then one rotation for the whole clip takes it away for good, and
the frame-by-frame alignment shakes less. No wave: nothing to do.

### 4. Sky on the seam: even it out from trusted pixels only

**What it does.** It builds a smooth correction of brightness and colour
not as one line along the seam but in **two dimensions**: along and
across the seam's band. It takes only pixels where both lenses are
surely honest: not on the very edge, not on sharp detail. It also
measures, for each direction, where the lens's edge starts to go bad.

**How it differs from what the author dropped.** His attempt was one line
per column, so right only in the middle of the band. He saw that
difference himself.

**Careful — patents.** OpenOSV says GoPro holds patents on estimating
flare from the difference between lenses (US11330208B2 and others, in
force until 2038), and turned its own part off for it. That's their
reading, not legal advice; it needs looking into before starting.

### Small findings, useful to him too

- **Night clips and proxies.** When the camera drops frames, the light
  `.LRF` copy records them wrongly: OpenOSV saw 3 s drift over 5 minutes.
  Playing from the proxy then drifts from the export.
- **6K mode.** A 3000 px lens is the centre of the 3840 calibration scaled
  by 0.794492, not simply 3000/3840. The plain scale pulls the seam apart
  by about 4°. The 100 fps mode (1920 px), which the author took from
  someone's pull request, hasn't had its scale checked either.

## Honestly, the limits

- OpenOSV's numbers come mostly from **one** clip of 65 frames (the camera
  on an aircraft's wing, with ND filters). They're ideas worth measuring,
  not proven results.
- We haven't measured any of this ourselves. There's no 360 camera of our
  own yet (about 2027-02), one public clip. The author has his whole
  Kyrkja hike, so it's easier for him to check than for us.
- OpenOSV's code is Apache-2.0. Code carried over word for word needs
  their credit in a NOTICE; an idea done in our own code doesn't, but
  mention them honestly.

## A draft for the author (not sent)

> **Seam ideas from OpenOSV that rOsmo360 hasn't tried**
>
> OpenOSV (Kemerd, Apache-2.0) is a C++/CUDA OSV plug-in for Premiere and
> Resolve. Reading its source, four seam ideas look new against your
> CLAUDE.md notes. Its numbers mostly come from one 65-frame wing clip, so
> treat them as leads, not results:
>
> 1. **Releasing DJI's mount mask where both lenses agree** (`MountMask.h`,
>    "Hide Mount: Auto"). Once per clip, on 9 frames, it renders both lenses
>    unmasked along the mask arc and scores 16-column windows by 2-D NCC
>    (±8° along, ±2° across). Where NCC ≥ 0.8 on texture the mask is
>    released; the windows it keeps are dilated by 12 columns. This is
>    your unexplored "tighter masks where the hump is wider than the stick",
>    and it may give the 0025 cap brim the overlap it lacks. It was tuned
>    on car mounts.
> 2. **The lens-protector correction.** `StreamMeta.7 extri_lens_mode`
>    (0 bare, 1 guards/ND, 2 underwater) with a field-angle correction
>    (+0.52° at 30°, +1.28° at 90°; their fit in `LensProtector.h`).
> 3. **Lens tilt.** Your φ test rules out roll about the lens axis; tilt
>    would show as an α-shift varying sinusoidally with azimuth. The cheap
>    check is to fit it to the median of your per-bin δ over many frames.
>    Their per-clip rotation fit (`LensAlign.h`) found 0.36° on an ND clip.
> 4. **A 2-D photometric seam field** from trusted pixels only, with a
>    measured usable rim (`PhotoSeam.h`). It differs from the dropped
>    `--veil` mainly in being 2-D across the band. OpenOSV flags GoPro flare
>    patents (US11330208B2) around overlap-based flare estimation.
>
> Also: the LRF time table is wrong on clips with dropped frames (they
> place frames by `FrameMeta.1` capture timestamps), and their 6K scale is
> 0.794492 (centre 3776 px), not 3000/3840. The 1920 px 100 fps scale may
> want the same check against `digital_focal_length`.
