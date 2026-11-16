# Commands: Omnix

*Made by `omnix commands`: the same text as `--help`. Don't edit it by hand.*

Omnix: edit home video and DJI Osmo 360 .OSV files (built on rOsmo360)

## `omnix rig`

Dump the camera rig (DJI Studio's default stitch geometry) as JSON

| Argument | What it is |
|---|---|
| `<INPUT>` |  |

## `omnix cache`

Where the cache of clips' telemetry and thumbnails is and what it holds (it makes clips open in a fraction of a second after the first time); `cache clear` empties it

## `omnix cache add`

Cache clips ahead of time, so they open at once later: each .OSV given, and every .OSV in the folders given (and folders inside)

| Argument | What it is |
|---|---|
| `<PATHS>` |  |
| `--force` | Read every clip afresh, even those already cached |

## `omnix cache move`

Move the cache to another folder (a disk with room, say), and keep it there from now on; OMNIX_CACHE, if set, still wins for a run

| Argument | What it is |
|---|---|
| `<FOLDER>` |  |
| `--default` | Back to the system's cache folder (~/.cache/omnix on Linux) |

## `omnix cache clear`

Delete the cached entries: all of them, or those unused for a while

| Argument | What it is |
|---|---|
| `--older-than <OLDER_THAN>` | Only entries no clip has used for this many days |

## `omnix imu`

Dump orientation (q), gyro (g) and accelerometer (a) samples as CSV

| Argument | What it is |
|---|---|
| `<INPUT>` |  |

## `omnix info`

The clip's length, frame rate, recording time and what it holds, as JSON

| Argument | What it is |
|---|---|
| `<INPUT>` |  |

## `omnix check`

List what in a project is likely a mistake, one line each (edit time, cut, what): cuts out of the order they were filmed in, a cut repeating footage the cut before shows, crossfades between cuts of one clip seconds apart (one scene: it stutters), pans faster than 30°/s (and whether keyframes or the camera turn the view), cuts under a second, and music ending before the edit or running long past it

| Argument | What it is |
|---|---|
| `<PROJECT>` |  |
| `--fast-pan <FAST_PAN>` | Pans faster than this, degrees a second over a second, are listed: they smear and are hard to follow (default: 30) |
| `--variant <VARIANT>` | Use this alternative calibration (see `rig`'s `variants`) |
| `--stabilize <STABILIZE>` | Stabilisation mode [default: each cut's own, else horizon] (one of: off, horizon, level, lock) |
| `--smooth <SMOOTH>` | Heading smoothing window for `horizon`, in ms either side (default: 500) |
| `--imu-axes <IMU_AXES>` | How the IMU's axes sit relative to the rig's (default: x180) (one of: identity, x180, y180, z180) |
| `--world-z-down` | Treat the attitude's world z as pointing down rather than up |
| `--swap-lenses` | Give each lens the other's video track (see `export --help`) |

## `omnix autocolour`

Auto colour: each cut measured over a dozen frames across it (its LUT on), and its levels, exposure and white balance set to put it right; a line each says what was measured and set

| Argument | What it is |
|---|---|
| `<PROJECT>` |  |
| `--cuts <CUTS>` | Only the cuts with these ids, as `describe` says them (all if none) |
| `--levels` | The levels alone (black and white points) |
| `--ops` | Print the ops it would apply (an ops file for `apply`) instead of applying them; what it measured goes to stderr |
| `-o, --output <OUTPUT>` |  |

## `omnix normalize`

Bring each cut's sound to -16 LUFS: measure what it plays over its own stretch and give it the gain (the export's limiter holds the peaks). Prints a line per cut; writes the project with -o

| Argument | What it is |
|---|---|
| `<PROJECT>` |  |
| `--cuts <CUTS>` | Only the cuts with these ids, as `describe` says them (all if none) |
| `--track <TRACK>` | Every clip on this sound track instead of the cuts: music, sfx, sfx2 or voice |
| `--even` | Even out each cut's sound first: quiet stretches lifted, loud ones left alone |
| `--ops` | Print the ops it would apply (an ops file for `apply`) instead of applying them; what it measured goes to stderr |
| `-o, --output <OUTPUT>` |  |

## `omnix mixcheck`

Check the sound against a platform's loudness and the mixing rules: each clip measured as the export makes it, the whole film, and what to fix (music too loud under speech, a sound effect over it, quiet speech, the film off the target)

| Argument | What it is |
|---|---|
| `<PROJECT>` |  |
| `--target <TARGET>` | The platform's target, LUFS integrated (YouTube -14) (default: -14) |
| `--every <EVERY>` | Also the whole film's loudness every this many seconds |

## `omnix beats`

A song's beats: a line per hit, its time in the file and its strength (0..1 of the song's strongest), those at --threshold or stronger

| Argument | What it is |
|---|---|
| `<SONG>` |  |
| `--threshold <THRESHOLD>` | (default: 0) |

## `omnix duck`

Lower the music under speech (auto-duck): find where cleaned voices and Voice takes speak and put volume points on the music's line, this far down there. Prints where; writes the project with -o

| Argument | What it is |
|---|---|
| `<PROJECT>` |  |
| `--depth <DEPTH>` | How far down, dB (default: 18) |
| `--ops` | Print the ops it would apply (an ops file for `apply`) instead of applying them; what it measured goes to stderr |
| `-o, --output <OUTPUT>` |  |

## `omnix apply`

Apply operations to a project: a JSON list of them, as the player makes them (`docs/omnix/OPS-API.md`), one step, all or none. Prints what each did; writes the project with -o. Without -o it is a dry run: what they would do, or which is refused and why

| Argument | What it is |
|---|---|
| `<PROJECT>` |  |
| `<OPS>` |  |
| `-o, --output <OUTPUT>` |  |

## `omnix describe`

What a project holds, by id: the cuts, sound clips and overlays that operations name

| Argument | What it is |
|---|---|
| `<PROJECT>` |  |

## `omnix card`

Makes an edit's card frames (the start pages' project cards): 16 frames of the film, kept beside its trip's data (or in Omnix's cache for an edit outside a trip); prints where

| Argument | What it is |
|---|---|
| `<EDIT>` |  |

## `omnix keys`

Every key and gesture of the window, as Markdown tables (the guide's reference page, the F1 window's list)

## `omnix commands`

Every command and its arguments, as Markdown (the guide's reference page; the same text as --help)

## `omnix ops-schema`

The JSON Schema of an ops file for `apply`: every operation, its fields and what they mean (for an agent's tools, or an editor)

## `omnix looks`

The looks an operation can set (a join's, an edge's, a cut's style): every one by its key, with its settings (key, range, default, choices), and the saved transition templates; with --template NAME, that template as a `template` op takes it; with --save-template NAME --project FILE --cut ID, the join before that cut (its overlap and the edges on both sides) saved as NAME

| Argument | What it is |
|---|---|
| `--template <TEMPLATE>` |  |
| `--save-template <SAVE_TEMPLATE>` |  |
| `--project <PROJECT>` |  |
| `--cut <CUT>` |  |

## `omnix install`

Omnix's icon and a desktop entry, in the menu and on the desktop, starting this very binary (Linux)

## `omnix blank`

Makes the black picture a blank cut uses (`Blank.png` in DIR, kept if there) and prints its path, for a cut `add` op: a gap in the edit for a title, an overlay or a sound

| Argument | What it is |
|---|---|
| `<DIR>` |  |

## `omnix from-sheet`

Makes an edit from a Dailix project's marking (its folder): clips rated ✓ or ?, their segments as cuts (a clip with none whole), the voice chosen, flags inside a cut as its markers; written as a new video of the trip (`videos/<day>/edit.json`), its path printed

| Argument | What it is |
|---|---|
| `<PROJECT>` |  |
| `--day <DAY>` | One day only (YYYY-MM-DD) |
| `--part <PART>` | One part of the trip only (its name): its clips from any day, those not rated yet too, junk left out |
| `--empty` | An empty edit, to build from the bin: of the day (--day), or of every day |

## `omnix stitch`

Stitch one frame to an equirectangular PNG

| Argument | What it is |
|---|---|
| `<INPUT>` |  |
| `-t, --time <TIME>` | Time in seconds (default: 0) |
| `-o, --output <OUTPUT>` | (default: equirect.png) |
| `-w, --width <WIDTH>` | Output width; height is width / 2 (default: 4096) |
| `--variant <VARIANT>` | Use this alternative calibration (see `rig`'s `variants`) instead of the default |
| `--only <ONLY>` | Render only this lens (0 or 1), for checking overlap alignment |
| `--stabilize <STABILIZE>` | Stabilisation mode, from the camera's attitude data [default: off for stitch and render, horizon for export and preview] (one of: off, horizon, level, lock) |
| `--smooth <SMOOTH>` | Heading smoothing window for `horizon`, in ms either side (default: 500) |
| `--imu-axes <IMU_AXES>` | How the IMU's axes sit relative to the rig's (default: x180) (one of: identity, x180, y180, z180) |
| `--world-z-down` | Treat the attitude's world z as pointing down rather than up |
| `--no-rs` | Skip rolling-shutter correction |
| `--exposure` | Even out the lenses' exposure at the seam. Off by default: where the sun veils one lens it dulls the other to meet it |
| `--no-align` | Skip shifting the lenses to line up close subjects at the seam |
| `--no-cut` | Blend everywhere instead of cutting where the lenses disagree |
| `--swap-lenses` | Give each lens the other's video track. Which track a lens was recorded to isn't in the clip (telemetry-parser goes by the camera model), and swapped back-to-back lenses still stitch cleanly, only half a turn round the stick: horizon mode then swings by twice the tilt and stick follow looks away from the holder (issue #4) |

## `omnix play`

Play clips in a window with free look, stabilisation and rolling-shutter correction.

← / →: a frame · editing: Shift+← / →: a second, ↑ / ↓: the previous / next cut or marker, Home / End · framing: ↑ / ↓: pitch, Shift+← / →: yaw · drag: look around · right-drag or Q/E: roll · scroll: zoom · space: play/pause · , .: frame step · S: stabilisation · R: rolling shutter · P: projection · M: a marker · Shift+M: mute · 0: reset view · F: fullscreen · Ctrl+O: open · K: keyframe the view here · [ ]: previous/next keyframe · Del: delete keyframe · J: its ease · C: split the cut · D: select the cut under the playhead · Shift+Del: delete the selected cut · Alt+[ / Alt+]: trim its start / end to the playhead · Alt+← / Alt+→: move it one place · Ctrl+Z / Ctrl+Shift+Z: undo / redo · Ctrl+= / Ctrl+− / Ctrl+0: zoom the timeline in / out / to the whole edit (or scroll on it)

| Argument | What it is |
|---|---|
| `<INPUT>` | Clips to open at start, one cut each in this order, or a saved project (.json); otherwise use the Open buttons |
| `--osv-size <OSV_SIZE>` | Stream the OSV itself at this size (pixels square) instead of the LRF proxy [default: the last source used] |
| `--hwaccel <HWACCEL>` | Video decoder; the first that works, in this order, if omitted (one of: vulkan, vaapi, cuda, videotoolbox, d3d11va, software) |
| `--stabilize <STABILIZE>` | Stabilisation mode [default: the last one used, at first horizon] (one of: off, horizon, level, lock) |
| `--imu-axes <IMU_AXES>` | (default: x180) (one of: identity, x180, y180, z180) |
| `--world-z-down` |  |

## `omnix seam`

Report how well the seam alignment works over a span: what ghosting is left (lens disagreement across the blend band) and how much the shift wobbles from frame to frame

| Argument | What it is |
|---|---|
| `<INPUT>` |  |
| `-s, --start <START>` | Start time in seconds (default: 0) |
| `-d, --duration <DURATION>` | Duration in seconds (default: 5) |
| `--dump <DUMP>` | Write the first frame's seam strips, differences and shift field as images into this directory |

## `omnix render`

Stitch a span of the clip to an equirectangular MP4

| Argument | What it is |
|---|---|
| `<INPUT>` |  |
| `-s, --start <START>` | Start time in seconds (default: 0) |
| `-d, --duration <DURATION>` | Duration in seconds; the rest of the clip if omitted |
| `-o, --output <OUTPUT>` | (default: equirect.mp4) |
| `-w, --width <WIDTH>` | Output width; height is width / 2 (default: 4096) |
| `--variant <VARIANT>` | Use this alternative calibration (see `rig`'s `variants`) instead of the default |
| `--only <ONLY>` | Render only this lens (0 or 1), for checking overlap alignment |
| `--stabilize <STABILIZE>` | Stabilisation mode, from the camera's attitude data [default: off for stitch and render, horizon for export and preview] (one of: off, horizon, level, lock) |
| `--smooth <SMOOTH>` | Heading smoothing window for `horizon`, in ms either side (default: 500) |
| `--imu-axes <IMU_AXES>` | How the IMU's axes sit relative to the rig's (default: x180) (one of: identity, x180, y180, z180) |
| `--world-z-down` | Treat the attitude's world z as pointing down rather than up |
| `--no-rs` | Skip rolling-shutter correction |
| `--exposure` | Even out the lenses' exposure at the seam. Off by default: where the sun veils one lens it dulls the other to meet it |
| `--no-align` | Skip shifting the lenses to line up close subjects at the seam |
| `--no-cut` | Blend everywhere instead of cutting where the lenses disagree |
| `--swap-lenses` | Give each lens the other's video track. Which track a lens was recorded to isn't in the clip (telemetry-parser goes by the camera model), and swapped back-to-back lenses still stitch cleanly, only half a turn round the stick: horizon mode then swings by twice the tilt and stick follow looks away from the holder (issue #4) |

## `omnix export`

Render a framed (reframed) video with sound: a clip with a fixed view or keyframed views from a JSON file, or a project (.json) of cuts from several clips, each with its own keyframes.

For a clip without -s/-d, the keyframes' span is rendered. Stabilisation defaults to horizon.

| Argument | What it is |
|---|---|
| `<INPUT>` | A clip (.OSV), or a project (.json): {"aspect": "9:16", "cuts": [{"input": "a.OSV", "in": 28, "out": 37, "keyframes": [...]}, ...]} |
| `--reframe <REFRAME>` | Keyframed views as JSON: {"aspect": "9:16", "keyframes": [{"t": 12, "yaw": 40, "pitch": -5, "fov": 80, "projection": "perspective", "ease": "smooth"}, ...]}. Keyframes may leave out what doesn't change |
| `--view <VIEW>` | A fixed view: YAW,PITCH[,ROLL[,FOV]] in degrees |
| `--projection <PROJECTION>` | Projection for --view (default: perspective) (one of: perspective, little-planet, equirect) |
| `-s, --start <START>` | Start time in seconds |
| `-d, --duration <DURATION>` | Duration in seconds |
| `-r, --resolution <RESOLUTION>` | Picture height for landscape, width for portrait: 1080p is 1920×1080 at 16:9 and 1080×1920 at 9:16 (default: 1080p) (one of: 720p, 1080p, 1440p, 2160p) |
| `-a, --aspect <ASPECT>` | Shape, W:H (16:9, 9:16, 1:1, 4:5, ...); else the reframe's, else 16:9 |
| `--size <SIZE>` | Exact size, WIDTHxHEIGHT, instead of a resolution and aspect |
| `-o, --output <OUTPUT>` | The file to write; with --clips, the folder for the clips. [default: clip.mp4, clip.mov for --format resolve, or the project's or clip's name for --clips] |
| `--format <FORMAT>` | mp4: H.264 and AAC, plays anywhere. resolve: 10-bit DNxHR HQX and PCM in MOV, which DaVinci Resolve's free Linux version can read (about 28 MB a second at 1080p) (default: mp4) (one of: mp4, resolve) |
| `--clips` | Write each cut to its own file for another editor, named after the project: "&lt;name> - 001.mp4" and on, in the folder -o; and each music clip as "&lt;name> - music 001.wav", each sound effect as "&lt;name> - sfx 001.wav" (48 kHz 24-bit, unfaded, at the file's own level) |
| `--handles <HANDLES>` | With --clips, seconds of extra footage either side of each cut (and song either side of each music clip), where there is some, for the other editor's transitions (default: 0) |
| `--graded` | With --clips, apply the cuts' colour effects too (the whole edit always has them; left off, the clips are as shot, to grade in the other editor) |
| `--loudness <LOUDNESS>` | Bring the whole film's sound to this loudness, LUFS integrated (YouTube and most streaming: -14; Apple and podcasts: -16; broadcast: -23). Not with --clips |
| `--draft` | Stitch from the low-resolution proxy (LRF): a quick, soft draft |
| `--cpu` | Stitch on the CPU, the reference the GPU shader must match. Without a usable GPU this happens anyway |
| `--variant <VARIANT>` | Use this alternative calibration (see `rig`'s `variants`) instead of the default |
| `--only <ONLY>` | Render only this lens (0 or 1), for checking overlap alignment |
| `--stabilize <STABILIZE>` | Stabilisation mode, from the camera's attitude data [default: off for stitch and render, horizon for export and preview] (one of: off, horizon, level, lock) |
| `--smooth <SMOOTH>` | Heading smoothing window for `horizon`, in ms either side (default: 500) |
| `--imu-axes <IMU_AXES>` | How the IMU's axes sit relative to the rig's (default: x180) (one of: identity, x180, y180, z180) |
| `--world-z-down` | Treat the attitude's world z as pointing down rather than up |
| `--no-rs` | Skip rolling-shutter correction |
| `--exposure` | Even out the lenses' exposure at the seam. Off by default: where the sun veils one lens it dulls the other to meet it |
| `--no-align` | Skip shifting the lenses to line up close subjects at the seam |
| `--no-cut` | Blend everywhere instead of cutting where the lenses disagree |
| `--swap-lenses` | Give each lens the other's video track. Which track a lens was recorded to isn't in the clip (telemetry-parser goes by the camera model), and swapped back-to-back lenses still stitch cleanly, only half a turn round the stick: horizon mode then swings by twice the tilt and stick follow looks away from the holder (issue #4) |

## `omnix preview`

Stitch evenly spaced moments onto one contact-sheet PNG, each tile labelled with its time, to skim a clip. Tiles show the whole sphere, marked at yaw −90/0/90° and the horizon, unless a framing is given.

Given a project (.json), one row per cut (more with --every), each tile framed as the export frames it and labelled with the cut's number and the clip time.

Uses the proxy (LRF) unless --full. Stabilisation defaults to horizon.

| Argument | What it is |
|---|---|
| `<INPUT>` |  |
| `--reframe <REFRAME>` | Keyframed views as JSON: {"aspect": "9:16", "keyframes": [{"t": 12, "yaw": 40, "pitch": -5, "fov": 80, "projection": "perspective", "ease": "smooth"}, ...]}. Keyframes may leave out what doesn't change |
| `--view <VIEW>` | A fixed view: YAW,PITCH[,ROLL[,FOV]] in degrees |
| `--projection <PROJECTION>` | Projection for --view (default: perspective) (one of: perspective, little-planet, equirect) |
| `-s, --start <START>` | Start time in seconds |
| `-d, --duration <DURATION>` | Duration in seconds |
| `--every <EVERY>` | Seconds between tiles; else 24 tiles, or the keyframes' times |
| `--tile <TILE>` | Tile width in pixels (default: 640) |
| `--columns <COLUMNS>` | (default: 4) |
| `-o, --output <OUTPUT>` | (default: preview.png) |
| `--full` | Stitch from the full-resolution lens tracks instead of the proxy |
| `--variant <VARIANT>` | Use this alternative calibration (see `rig`'s `variants`) instead of the default |
| `--only <ONLY>` | Render only this lens (0 or 1), for checking overlap alignment |
| `--stabilize <STABILIZE>` | Stabilisation mode, from the camera's attitude data [default: off for stitch and render, horizon for export and preview] (one of: off, horizon, level, lock) |
| `--smooth <SMOOTH>` | Heading smoothing window for `horizon`, in ms either side (default: 500) |
| `--imu-axes <IMU_AXES>` | How the IMU's axes sit relative to the rig's (default: x180) (one of: identity, x180, y180, z180) |
| `--world-z-down` | Treat the attitude's world z as pointing down rather than up |
| `--no-rs` | Skip rolling-shutter correction |
| `--exposure` | Even out the lenses' exposure at the seam. Off by default: where the sun veils one lens it dulls the other to meet it |
| `--no-align` | Skip shifting the lenses to line up close subjects at the seam |
| `--no-cut` | Blend everywhere instead of cutting where the lenses disagree |
| `--swap-lenses` | Give each lens the other's video track. Which track a lens was recorded to isn't in the clip (telemetry-parser goes by the camera model), and swapped back-to-back lenses still stitch cleanly, only half a turn round the stick: horizon mode then swings by twice the tilt and stick follow looks away from the holder (issue #4) |

## `omnix frame`

One frame of a project's edit at --at seconds, as the export makes it: framed, graded with its LUT, styled, its edge look, faded, with overlays and text. From the proxy unless --full

| Argument | What it is |
|---|---|
| `<PROJECT>` |  |
| `--at <AT>` | Edit time, seconds |
| `--width <WIDTH>` | (default: 1280) |
| `-o, --output <OUTPUT>` | (default: frame.png) |
| `--full` |  |
| `--variant <VARIANT>` | Use this alternative calibration (see `rig`'s `variants`) instead of the default |
| `--only <ONLY>` | Render only this lens (0 or 1), for checking overlap alignment |
| `--stabilize <STABILIZE>` | Stabilisation mode, from the camera's attitude data [default: off for stitch and render, horizon for export and preview] (one of: off, horizon, level, lock) |
| `--smooth <SMOOTH>` | Heading smoothing window for `horizon`, in ms either side (default: 500) |
| `--imu-axes <IMU_AXES>` | How the IMU's axes sit relative to the rig's (default: x180) (one of: identity, x180, y180, z180) |
| `--world-z-down` | Treat the attitude's world z as pointing down rather than up |
| `--no-rs` | Skip rolling-shutter correction |
| `--exposure` | Even out the lenses' exposure at the seam. Off by default: where the sun veils one lens it dulls the other to meet it |
| `--no-align` | Skip shifting the lenses to line up close subjects at the seam |
| `--no-cut` | Blend everywhere instead of cutting where the lenses disagree |
| `--swap-lenses` | Give each lens the other's video track. Which track a lens was recorded to isn't in the clip (telemetry-parser goes by the camera model), and swapped back-to-back lenses still stitch cleanly, only half a turn round the stick: horizon mode then swings by twice the tilt and stick follow looks away from the holder (issue #4) |

## `omnix scopes`

The scopes of one frame of a project's edit at --at seconds, as `frame` draws it: its darkest and brightest, what's crushed and blown, the luma in sixteen bands

| Argument | What it is |
|---|---|
| `<PROJECT>` |  |
| `--at <AT>` |  |
| `--full` |  |
| `--variant <VARIANT>` | Use this alternative calibration (see `rig`'s `variants`) instead of the default |
| `--only <ONLY>` | Render only this lens (0 or 1), for checking overlap alignment |
| `--stabilize <STABILIZE>` | Stabilisation mode, from the camera's attitude data [default: off for stitch and render, horizon for export and preview] (one of: off, horizon, level, lock) |
| `--smooth <SMOOTH>` | Heading smoothing window for `horizon`, in ms either side (default: 500) |
| `--imu-axes <IMU_AXES>` | How the IMU's axes sit relative to the rig's (default: x180) (one of: identity, x180, y180, z180) |
| `--world-z-down` | Treat the attitude's world z as pointing down rather than up |
| `--no-rs` | Skip rolling-shutter correction |
| `--exposure` | Even out the lenses' exposure at the seam. Off by default: where the sun veils one lens it dulls the other to meet it |
| `--no-align` | Skip shifting the lenses to line up close subjects at the seam |
| `--no-cut` | Blend everywhere instead of cutting where the lenses disagree |
| `--swap-lenses` | Give each lens the other's video track. Which track a lens was recorded to isn't in the clip (telemetry-parser goes by the camera model), and swapped back-to-back lenses still stitch cleanly, only half a turn round the stick: horizon mode then swings by twice the tilt and stick follow looks away from the holder (issue #4) |

## `omnix scan`

Summarise the clip over time as CSV, to find moments worth a look: the camera's heading and tilt, how fast it turns or shakes, and how loud it is.

Given several clips or folders (every .OSV in them and the folders inside), writes a CSV per clip into the folder --output, named after the clip, a few clips at a time.

| Argument | What it is |
|---|---|
| `<INPUTS>` |  |
| `-o, --output <OUTPUT>` | The folder for a CSV per clip; one clip without it goes to the standard output |
| `--jobs <JOBS>` | Clips scanned at once into --output (default: 4) |
| `--every <EVERY>` | Seconds per row (default: 1) |
| `--image` | Add columns from the picture: sky (% of the upper half that is sky), skyline (its median elevation, degrees) and open (% of directions with a view out to the horizon). Stitches the proxy's keyframes small, which takes seconds to a minute where plain scan reads only telemetry |

## `omnix track`

Track a coloured subject (a jacket, a tent) through a clip, as CSV for keyframes to follow ("follow": "track.csv"): per sample its direction in the camera (x, y, z), its yaw and pitch as a level view would look at it (directions in the world, like scan's heading), and its size in % of the sphere. Empty where it was lost

| Argument | What it is |
|---|---|
| `<INPUT>` |  |
| `--colour <COLOUR>` | The subject's colour, #rrggbb (from a preview or a photo of it) |
| `--tolerance <TOLERANCE>` | How far the hue may differ, degrees either side (default: 18) |
| `--near <NEAR>` | Where the subject is at the start, as level yaw,pitch (it may not be the biggest patch of its colour); else it starts from that |
| `-s, --start <START>` | Start time in seconds |
| `-d, --duration <DURATION>` | Seconds to track; to the end if omitted |
| `--every <EVERY>` | Seconds between samples (default: 0.1) |
