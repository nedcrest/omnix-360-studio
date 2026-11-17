# Install and first run

Omnix and Dailix run on Linux. Each comes as a release: an archive with
an install script, or an AppImage. Install both: Dailix hands its edits
to Omnix, and each finds the other beside it.

## What you need

- **A graphics card with Vulkan** and up-to-date drivers: Omnix plays,
  decodes and exports on it.
- **ffmpeg and ffprobe** on your `PATH`. On Debian or Ubuntu:
  `sudo apt install ffmpeg`.
- **ALSA** for sound (PipeWire and PulseAudio work through it).
- To build: **Rust** 1.88 or newer ([rustup.rs](https://rustup.rs)), and
  on Debian or Ubuntu `sudo apt install pkg-config libasound2-dev`.

## Install from the releases

Download Omnix from [its releases
page](https://github.com/nedcrest/omnix-360-studio/releases/latest) and
Dailix from [its](https://github.com/nedcrest/dailix/releases/latest):

- **The archives**: unpack each and run its `./install.sh`. It puts
  `omnix` (or `dailix`) in `~/.local/bin`, with its icon and a menu entry
  (`--prefix /usr/local` as root installs it for everyone; `--uninstall`
  takes it away). Run on its own, an `install.sh` downloads the latest
  release first:

  ```sh
  curl -fsSL https://github.com/nedcrest/omnix-360-studio/releases/latest/download/install.sh | sh
  curl -fsSL https://github.com/nedcrest/dailix/releases/latest/download/install.sh | sh
  ```

- **The AppImages**: put `Omnix-x86_64.AppImage` and
  `Dailix-x86_64.AppImage` in one folder, make them executable
  (`chmod +x`) and run them; Dailix's «→ Omnix» finds Omnix's beside it.

## Build both from source

The two live in two repositories, cloned side by side; Dailix builds
against Omnix's folder, which has to be named `omnix-360-studio`.

```sh
git clone https://github.com/nedcrest/omnix-360-studio
git clone https://github.com/nedcrest/dailix
cd omnix-360-studio && cargo build --release && cd ..
cd dailix && cargo build --release && cd ..
```

The programs are `omnix-360-studio/target/release/omnix` and
`dailix/target/release/dailix`. To put each in your menu, with its icon:

```sh
omnix-360-studio/target/release/omnix install
dailix/target/release/dailix install
```

Each finds the other by itself: on your `PATH`, or as a build in the
repository beside its own. (`OMNIX_BIN` and `DAILIX_BIN` name one
somewhere else.)

## Dailix's first run

Dailix asks where to keep your footage, in three folders:

- **Trips**: a folder for each trip, with its clips, its marking and its
  edits. This is the one Dailix needs.
- **TMP** (optional): where footage lands first, from cards and phones.
- **Sources** (optional): the originals, kept as they came off the
  camera and never changed.

![Dailix's welcome: Trips, TMP and Sources, each with its folder and a Choose button, and Use these folders.](img/dailix-welcome.png)

1. To keep a folder somewhere else, select **Choose…** beside it.
2. Select **Use these folders**. Dailix makes the ones that aren't
   there yet.

You can change them later, in **☰** > **Settings** (or **Ctrl+,**).
Then copy your videos into the TMP folder and select **TMP** on
**Trips**: the sorter makes your first trip from them (or **+ New trip**,
then **⋯** > **Add videos…** on its page). The
[tutorial](tutorial-0-bring-in.md) goes on from there.

## Omnix's first run

Omnix opens on its start screen. Once you've made edits, they are shown
here as cards, the latest first.

![Omnix's start screen: New, Open clips, Open project, Add music and Settings.](img/omnix-start.png)

- **New…**: an edit in a trip (the trip's clips in its bin), a new
  trip, or an edit on its own.
- **Open clips…**: videos, pictures or Osmo 360 clips, straight into a
  new edit.
- **Open project…**: an edit saved before.
- **Add music…**: start from a song, and cut the pictures to it.

An edit made in Dailix (its **→ Omnix**) opens in Omnix by itself.
