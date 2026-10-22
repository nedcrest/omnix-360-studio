# Omnix 360 Studio

A Linux-native studio for 360° video — the equivalent of Insta360 Studio and
DJI Studio, which their vendors do not ship for Linux. **DJI only** (decided
2026-10-22: the user's gear is DJI — Action 2, Neo 2, Osmo Mobile 7P — and the
360 camera will be an Osmo 360 / Osmo 360 II). Insta360 is out of scope.

Built as a local fork of [rOsmo360](https://github.com/ravelund/rOsmo360),
which already covers the Osmo 360 (stitching, gyro stabilisation, reframing,
timeline). Also meant as a simple home-video editor: 360 is one kind of clip
beside ordinary phone / Action 2 / Neo 2 footage.

## Why

- Insta360 Studio and DJI Studio exist only for Windows and macOS.
- DaVinci Resolve (free) on Linux cannot decode H.264/H.265 (patent licensing:
  on Win/Mac it borrows the OS decoder, on Linux there is none), so the
  existing Resolve 360 plugins are out of reach without Resolve Studio.
- Personal project first; publish a polished version on GitHub later.

## Core idea: virtual cameras

A 360 clip is treated as a *set*, not a clip to rotate. Inside it we place any
number of **virtual cameras** — each with its own yaw/pitch/roll/FOV path over
time — and edit between them like a multicam shoot. AI later places and drives
those cameras (follow the speaker, follow an object, pick the interesting
direction).

## Principles

- **No vendor SDKs, no registrations.** Everything — file parsing, lens
  calibration, stitching — is our own open code, so anyone can build and run it.

## Scope

### Stage 1 — parity with the vendor studios
- Osmo 360 II `.osv` support (resolution, calibration, 8K50)
- Gyro stabilisation for ordinary DJI clips (Action 2, Neo 2) — telemetry-parser reads them
- Gyro-based stabilization and horizon lock (via Gyroflow's `telemetry-parser`)
- Reframing with keyframes and curves; projections: rectilinear, tiny planet, fisheye
- Real-time GPU preview; proxies for 8K
- Colour: Log profiles (I-Log, D-Log M), `.cube` LUTs, basic grading
- Hardware-encoded export (NVENC)

### Transitions
- Preset transitions with previews, parameters and project templates; T key,
  orange zones, overlap vs edge (out/in) detected automatically; 360-only
  looks (whip pan, little-planet zoom). Spec: [TRANSITIONS.md](TRANSITIONS.md).

### Stage 2+ — AI assisted editing
- Speaker-follow: spatial audio direction of arrival + active speaker detection
- Click/text tracking ("follow the dog") — SAM 2 + detector
- Transcript-based editing (whisper + diarization)
- Visual search over footage (CLIP/SigLIP)
- AI rough cut mixing reframed 360 and ordinary flat clips
- Optional cloud models via user-supplied Claude / OpenAI key

### Bridges
- Export flat clips, `.kdenlive` (MLT XML) or OpenTimelineIO timelines
- Later: an MLT filter so reframes stay editable inside Kdenlive

## Stack (planned)

| Part | Choice | Why |
|---|---|---|
| Core | Rust | speed for 8K frames; reuse Gyroflow crates; compiler catches bugs early |
| GPU | wgpu (Vulkan) | sphere → flat reprojection per frame |
| Decode / encode | FFmpeg + NVDEC / NVENC | hardware H.265 10-bit |
| UI | Qt/QML | proven by Gyroflow, native to KDE |
| AI | Python sidecar over local API | the model ecosystem lives there |

Licence: MIT OR Apache-2.0, as upstream (ffmpeg is called as a separate program).

## Target machine

NVIDIA RTX 3060 12 GB (driver 580). Note: Ampere has no AV1 *encoder*.

## Roadmap

0. **Spike** — measure 8K H.265 decode on RTX 3060; free-look equirect preview
   with the mouse; read gyro from a sample `.insv` / `.osv`.
1. **MVP** — virtual cameras with keyframes, horizon lock, stabilization, LUT,
   proxies, export.
2. **Osmo 360 II** — once sample files exist: check calibration and decode
   speed at 8K50.
3. **Local AI** — tracking, speaker-follow, transcript.
4. **AI editing** — flat clips, cloud models, Kdenlive bridge.

## Open questions

- Does telemetry-parser read Osmo 360 II calibration, or does it need adding?
- Can the RTX 3060 decode 8K50 HEVC 10-bit in real time, or are proxies mandatory?
- No 360 camera for ~4 months: samples from public sources (Osmo 360 sample from the Bennetts review works).
