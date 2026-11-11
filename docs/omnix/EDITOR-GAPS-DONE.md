

### 99b. Sync lock on text and overlay lanes — small (user, 2026-11-11)

The user: voice-over read over a video with useless pauses, then cut
together with the video to drop the empty moments — and the same for
overlays. The voice-over half works already: the Voice lane has sync
lock (99): lock it, cut the pauses out of the picture, the take is cut
with them. To add: a lane's «Sync lock» on text and overlay lanes too;
an item follows the picture it lay under (moved with it; shortened where
picture under it is taken away). Whether text is cut in two at an edit
like a song, or only shortened, to decide with him (a title in two
pieces is odd). Milestone «Зручний таймлайн».


**Built and checked by the user 2026-11-11** (the user chose: shortened, not cut
in two). A lane's sync (`overlay::Lane::sync`, `ItemOp::LaneSync`, its
menu on a right-click of its name with «Rename…», «· sync» in its name);
`follow_picture_items`: an item stays one, starts where its first
stretch still shown went, is as long as what's left under it, and goes
when nothing is; `Project::locked`/`follow_from` hold sound and item
lanes alike, so a drag follows from its start for both.
