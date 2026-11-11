

### 103. A cut's voice chosen in the edit, and its own sound as speech — small (user, 2026-11-11)

The user couldn't lower the music under the talk in the test edit. Found:
auto-duck takes speech only from a cut's cleaned voice (`Cut::audio`) and
Voice takes (`duck::sources`); a cut's own sound doesn't count (wind and
traffic as often as talk). The talking clip has three voices made in
Dailix (dnn6, dnn12, eq) but none chosen («Use»), and an edit takes the
chosen voice only when it's built from the sheet (`from_sheet`): one
chosen later never reaches an edit made before, and Omnix has no op to
set it. `omnix duck` on that edit finds speech only on the Voice take.
To do: (1) `CutOp::Voice { cut, voice }` and a «Voice» choice in the
clip-sound menu (its own, or each voice Dailix made for the clip); an
edit opened offers the sheet's newer choice; (2) a per-cut «its own
sound is speech» switch for a clean recording that needs no cleaning,
which `duck::sources` then counts. Milestone «Ефекти, колір і розумний
звук».


**Built and checked by the user 2026-11-11** (dnn12 on every cut of the day, the music ducked under the talk). Right-click a cut's sound → «Voice»:
its own sound, each cleaned voice Dailix made for the clip (the marking's
choice named, the one playing ticked), «Its own sound is speech»
(`from_sheet::voices`, `CutOp::Voice` with the voice's loudness over the
cut → its level, `CutOp::Speech`; `duck::sources` and the mix check count
it). Opening an edit offers the voices chosen in Dailix since it was
made, once (`Cut::sheet_voice`, `newer_voices`, `CutOp::SheetVoice`):
«Use them» (one undo step) or «Keep as it is»; Esc asks again next time.
