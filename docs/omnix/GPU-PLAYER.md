# The player's CPU: where it goes, GPU against CPU (gap 65)

Asked by the user on 2026-11-04: "the graphics card sits idle while
everything is ground on the CPU". Measured the same day on the dev PC:
an RTX 3060 (12 GB, driver 580), an i5-11400F (6 cores, 12 threads),
ffmpeg 6.1.1 (Ubuntu). No OSV footage on this machine, so 360 clips are
not measured here.

## The answer first

The decoding is not what loads the machine. While the test project plays,
and while the playhead is dragged, **the player's main thread (the
interface) runs at 100 % of a core**, and ffmpeg takes 5-15 %. Idle, both
are at 0. One core pinned is also what the user's system monitor showed
(the yellow line on 2026-11-04).

So the first fix is the main thread's drawing, not moving decoding to
the GPU. Moving the scaling to the GPU (below) is a second, smaller win,
for 4K and 2.7K files only.

## 1. The player itself, sampled per thread

`/proc/<pid>/task/*/stat` every second while the user played the test
project for ~20 s and then dragged the playhead for ~20 s
(`scratchpad/bench/watch.py`; perf is closed at `perf_event_paranoid`
4).

| Phase | Main thread | Other threads | ffmpeg |
|---|---|---|---|
| Idle | 0 % | 0 % | 0 % |
| Playing (1080p H.264, CUDA) | 95-102 % | 1-6 % | 3-35 % (spikes at cut changes) |
| Dragging the playhead | 60-100 % | 0-2 % | 0-15 % |

The main thread is iced's: it builds the view after every message and
draws every canvas. The timeline's canvases (cut row, clip sound, five
sound lanes with waveforms per two pixels, overlay and text lanes,
markers, the ruler) are redrawn from nothing on every frame shown and
every move of the pointer: no `canvas::Cache` is used anywhere, and
iced's wgpu backend tessellates each filled path on the CPU (lyon) when
it is drawn. Not yet measured: which canvas costs most, and how much the
view's rebuild (the `Timeline` struct cloned per message) adds.

The user saw the same from the other side: dragging the playhead is
smooth in a smaller window and lags in fullscreen (2026-11-04). The
decode doesn't change with the window; the drawing does: waveforms are
drawn a bar per two pixels on every sound lane, so a wider timeline is
more paths to tessellate on every frame.

Receiving frames from ffmpeg costs the player little: a test through the
player's own `source::spawn`, 150 frames at 30 fps, 1080p NV12 (93 MB/s),
took 3 % of a core (software decode) and 5 % (CUDA).

### Which part of the main thread (measured the same day)

A throwaway build timed each canvas's `draw`, the view's build and
`update`, a line a second; the user played and dragged the playhead in a
window, then fullscreen:

| | Window | Fullscreen |
|---|---|---|
| The lanes' canvas (`Marks::draw`), per frame | **40 ms** | **79 ms** |
| … of it the sound lanes (waveforms, lines) | 30 ms | 59 ms |
| … of it the clip sound lane | 10 ms | 19 ms |
| Frames it manages a second | ~24 | ~13 |
| Every other canvas (ruler, markers, overlay and text lanes, thumbnails, meter, handles, divider) | ≤ 0.2 ms each | ≤ 0.2 ms each |
| The view's build, the timeline's model | 0.1 ms | 0.1 ms |
| `update` | 0.1-5 ms | 0.1-5 ms |

So nearly all of it is the waveforms: drawn a bar per two pixels in
three layers on every lane, tessellated anew on every frame, twice the
work at twice the width; the frame rate halves in fullscreen, as the
user saw. Nothing else is worth touching first.

### After keeping the sound lanes (`8a2177d`)

The sound lanes and the clip sound lane drawn once into a `canvas::Cache`
and kept while what they show holds. The same sampling, the player run
the same way (a window, then fullscreen, playing and dragging):

| Phase (by the trace) | Main thread before | after |
|---|---|---|
| Playing, window | 95-102 % | 10-28 % |
| Dragging the playhead, window | 60-100 % | 10-17 % |
| Playing, fullscreen | ~100 % | 17-30 % |
| Dragging, fullscreen | ~100 % | 8-26 % |

Mean over the busy seconds 15 % of a core, at most 30 %; ffmpeg as
before (0-45 %, the peaks at cut changes).

## 2. Decoding: software against CUDA, as the player runs ffmpeg

The player's command (`ffmpeg::yuv`): decode, then for a GPU decode
`hwdownload,format=nv12`, then `scale=1920:H:flags=bilinear,format=nv12`
on the CPU, raw NV12 down a pipe. Vulkan Video crashes in this ffmpeg
(exit 139), so the probe picks CUDA. Ten seconds of video, as fast as it
goes (max) and at real time (rt, `-re`: what playing costs):

| File | Path | max fps | max CPU | rt CPU (% of a core) |
|---|---|---|---|---|
| 1080p H.264 (test clip) | software | 361 | 837 % | 67 % |
| | CUDA, CPU scale (today) | 286 | 67 % | 12 % |
| | CUDA, GPU scale | 300 | 66 % | 12 % |
| 4K H.264 50 Mb/s (made) | software | 154 | 722 % | 117 % |
| | CUDA, CPU scale (today) | 125 | 142 % | 68 % |
| | CUDA, GPU scale | 142 | 46 % | 15 % |
| 4K HEVC (made) | software | 182 | 767 % | 101 % |
| | CUDA, CPU scale (today) | 197 | 211 % | 73 % |
| | CUDA, GPU scale | 242 | 58 % | 14 % |
| 2.7K HEVC (DJI, Kos archive) | software | 58 | 293 % | 140 % |
| | CUDA, CPU scale (today) | 294 | 249 % | 63 % |
| | CUDA, GPU scale | 303 | 71 % | 15 % |

- For 1080p the GPU already decodes and nothing needs scaling: 12 % of a
  core, where software would take 67 %.
- For bigger files the CPU's scaling after the download is most of the
  cost: `scale_cuda=1920:H:format=nv12` before `hwdownload` cuts real-time
  CPU 4-5× (68 → 15 %, 73 → 14 %, 63 → 15 %) and downloads a quarter of
  the bytes. Today `scale_cuda` is used only when no size is asked for.
  It was slower once (the CLAUDE.md note, on the 3090 and an older
  ffmpeg); here it is faster. A change of a few lines in `ffmpeg::yuv`,
  with a GPU-against-CPU picture check.

## 3. Jumps of the playhead: the first frame at a new place

Best of three, ffmpeg started fresh as the player starts it for a jump:

| File | software | CUDA |
|---|---|---|
| test 1080p, keyframe every 8 s, at 7.9 s | 0.48 s, 3.7 s CPU | 0.73 s |
| its scrub copy (gap 63) | **0.05 s**, 0.07 s CPU | 0.46 s |
| DJI 2.7K HEVC, keyframe every 1 s | 0.89 s | 0.49 s |
| its scrub copy | **0.05 s** | 0.47 s |
| DJI's own LRF (720p, keyframe every 8 s) | 0.24 s | 0.51 s |
| 4K H.264, keyframe every 2 s | 0.64 s | 0.87 s |

A CUDA decoder takes ~0.45 s to start in a fresh ffmpeg, whatever the
file: for jumps a GPU decoder per jump is always slower than the scrub
copy in software, which is why gap 63 decodes the copy on the CPU. Only a
decoder kept open and seeked (in-process decoding, gap 52) would make
the GPU win here.

DJI cameras write their own LRF beside each MP4 (720p H.264), but with a
keyframe every 8 s it jumps 5× slower than our copy: not worth reading in
place of it.

## What to do, in order

1. ~~Measure the main thread~~: done, above: the waveforms.
2. ~~Cache the timeline's still layers~~: done for the sound lanes
   (`8a2177d`), above; the rest of the timeline costs under 1 ms. Was:
   (`canvas::Cache`): waveforms,
   blocks, thumbnails and headers redrawn only when the edit, the zoom or
   the window change; the playhead, the snap line and readouts in a thin
   layer of their own redrawn every frame. Expected to take the main
   thread from 100 % to a fraction while playing.
3. ~~Scale on the GPU~~: done (`b81a37b`): with CUDA, `scale_cuda` at the
   stored frame's size before `hwdownload`, the turn after on the small
   frame; checked against the software decode (within 3 levels of luma,
   turned and not).
4. Later: a Settings choice «Decoding: automatic / GPU / CPU» (gap 65),
   the preview's size (gap 64), and zero-copy (gap 52).
