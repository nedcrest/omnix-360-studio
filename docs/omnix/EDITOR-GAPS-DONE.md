

### 104. Dailix's «→ Omnix» opens the day's edit that's there — small (user, 2026-11-11)

«→ Omnix» (and «Empty day», and the trip's «all days») makes a new video
each time (`from_sheet::write` → a fresh `videos/<day>`, `-2`, `-3`…),
so the test trip piled up copies. The user: when the day already has
edits, a dialog lists them (name, when changed) — open one, or make a
new one; none, as now. A day's edits by name: `<day>` or `<day>-<n>`
(`all-days` likewise), a pure tested function. The trip page's
«Videos» list stays as it is.


**Built 2026-11-11 (Dailix `c1f34c1`), checked by the user the same day** (a lighter ground and frame under the pointer added on his word). When the day (or the trip, for
«all days») has videos made that way, a dialog lists them newest first
with when each was changed: a click opens one; «New from the marking»
(«New empty video» for «Empty day») makes another; Cancel or a click
outside does nothing. `dailix_sheet::project::videos_named`.
