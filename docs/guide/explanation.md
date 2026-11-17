# How it fits together

The tutorial shows what to press; this page is why things are as they
are.

## Trips, days, clips and edits

A **trip** is the unit of everything: one holiday, one weekend, one
event. It's a folder in your **Trips** place (see
[Settings and files](settings-and-files.md)) and holds:

- its **clips**, filed by the **day** they were filmed (the date in the
  file's name, else the date the camera wrote in it);
- its **marking**: what you rated, the stretches you marked, your notes,
  each day's story;
- its **edits**: as many films of it as you like (the whole trip, one
  day, a short for a phone), each with its exports.

Dailix and Omnix work on the same trip folder. Dailix is for looking
through and choosing; Omnix for editing. Neither keeps a database of
its own: everything is in the trip's folder, in files you can read.

The **originals** are never touched. A trip holds copies of the clips
(or of the stretches of them you added); the originals stay in
**Sources**, as they came off the camera.

## The way footage goes

Three places, and footage moves between them in one direction:

1. **TMP**: what came off the cards and phones, waiting. Nothing there
   is the only copy for long.
2. **Trips**: the TMP sorter copies each video into a trip, under the
   day it was filmed, and into the trip's **parts** if asked. A part is
   a list, not a folder: a clip is in its day once, and in as many
   parts as you like.
3. **Sources**, the archive: the originals, copied in by trip and day
   and checked by their content's hash (BLAKE3) read back from the copy.
   Only then can TMP let them go, to the trash, after the archive copy
   is read once more.

TMP keeps a ledger of what went where (`.dailix-filed.json`); a video
counts as filed only while its copy is in its trip, and as the same
video only while the file at its place in TMP is the same size, so a
new card's `DJI_0001` is never taken for an old one. Nothing in the
archive is written over or changed; the one thing Dailix moves there is
a copy it put under `_not-in-a-trip`, into its trip's day once the
video is filed.

## From the marking to an edit

Dailix's **→ Omnix** makes an edit from the marking, by fixed rules:

- clips rated **✓** (take) or **?** (think about it) go in; **✕** (junk)
  and unrated ones don't;
- a clip with segments goes in as those segments, each a cut with its
  role and note; one with none goes in whole;
- flags are notes to yourself, not material: a flag inside a segment
  becomes a marker on that cut;
- everything in the order it was filmed, day by day.

That's a first cut to work from, not a film. The marking stays in the
trip: **→ Omnix** again later offers to open the edit you have, or makes
another.

## A clip's voice

A camera's or a phone's own microphone hears the wind, the sea and the
traffic as well as whoever is talking. Dailix makes **cleaned voices** of
a clip: each a recipe (**dnn6**, the default, takes the noise down by up
to 6 dB; others go further, or shape the sound against wind), each made
from the original sound, kept beside the trip's clips, and deletable
without loss. You listen to them against the original, choose one with
**Use for the edit**, and the edits play it in that clip's cuts, at a
level measured from how loud it is.

In Omnix, right-click a cut's sound and choose **Voice** to change which
one it plays: its own sound, or any voice Dailix made. **Its own sound is
speech** says a clip's own sound is talk worth keeping clear (a clean
recording that needs no cleaning).

Omnix knows where people speak from those: the cleaned voices, the
recordings on the **Voice** lane, and the clips marked as speech. That's
what **Lower the music under speech** lowers the music under.

## Sound from the original

Lowering, normalising and evening out never stack. Each works from the
clip's original sound: pressing **Normalize…** a second time replaces the
first result rather than working on top of it. So you can try a setting,
change your mind, and never wear the sound down.

## Sync lock

Cutting to a song's beats has a trap: shorten a clip at the start and
every clip after it moves left, while the song stays put; every cut falls
off its beat. **Sync lock** (in a sound lane's menu: right-click its
name) ties a lane to the picture: while it's on, whatever makes the
picture shorter or longer at some point moves that lane's clips after
the point by the same amount, so what plays under each picture stays
under it. The text and overlay lanes have it too.

## Groups

Clips that belong together (a cut, its music hit and its title) can be
**grouped**: select them (**Shift**-click) and press **Ctrl+G**. A group
moves as one. A click on a grouped clip still selects that clip alone,
to trim it; **Alt**-click selects the whole group. **Ctrl+Shift+G**
undoes a clip's group.

## The cache

The first time Omnix opens a clip it reads it through: for a 360 clip
its motion data (for stabilisation) and thumbnails, for any clip its
waveform; for an ordinary video it also makes a small **quick copy**
with a key frame every few frames, so the playhead can be dragged
smoothly. All of that goes in the cache (`~/.cache/omnix`), so the clip
opens at once the next time. It can always be made again: clearing it
loses nothing but time. `omnix cache` says how big it is.
