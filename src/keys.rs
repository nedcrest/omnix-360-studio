//! Every key and gesture of Omnix's window, grouped: one list, read by
//! the shortcuts window (F1, `player/shortcuts.rs`), printed by `omnix
//! keys` and kept as the guide's reference page (`docs/guide/
//! keys-omnix.md`, checked by a test: one fact, one place). Dailix
//! renders its own list with `markdown` too. Keep it in step with
//! `player/keys.rs` and the timeline's presses.

/// The groups: a title, and its lines (keys, what they do).
pub type Groups<'a> = &'a [(&'a str, &'a [(&'a str, &'a str)])];

pub const GROUPS: &[(&str, &[(&str, &str)])] = &[
    (
        "Play and the playhead",
        &[
            ("Space", "play / pause"),
            ("Shift+Space", "play the range marked once (Shift and a drag on the lanes' empty stretch marks it, Esc lets go)"),
            ("← / →  or  , / .", "a frame back / on"),
            ("Shift+← / →", "a second back / on (editing)"),
            ("↑ / ↓", "the previous / next cut or marker (editing)"),
            ("Home / End", "the edit's start / end (editing)"),
            ("Shift+M", "mute"),
            ("F", "full screen"),
        ],
    ),
    (
        "Cuts",
        &[
            ("C", "split at the playhead (a sound clip if one is selected)"),
            ("T", "transitions: the join nearest the playhead"),
            ("M", "a marker at the playhead"),
            ("Alt+[ / Alt+]", "trim the start / end to the playhead"),
            ("D", "select the cut under the playhead"),
            ("Alt+← / Alt+→", "move the selected cut one place; a selected song 1 ms (with Shift 10 ms), to line a beat up"),
            ("Alt+↑ / Alt+↓", "the selected text or picture a lane up / down (a new lane on top)"),
            ("Del", "delete what's selected (Shift+Del: the cut)"),
            ("Ctrl+G / Ctrl+Shift+G", "group the clips selected / undo the selected clip's group"),
            ("Esc", "cancel a drag under way"),
            ("Ctrl+Z / Ctrl+Shift+Z", "undo / redo"),
        ],
    ),
    (
        "On the timeline (mouse)",
        &[
            ("drag an edge", "trim the selected clip (yellow: more footage, red: its end)"),
            ("drag a clip", "move it"),
            ("Shift+click a clip", "add it to the selection (again: take it away)"),
            ("Alt+click a grouped clip", "select its whole group (a click: the clip alone)"),
            ("Shift+drag a cut's start", "crossfade: out, onto the cut before"),
            ("Shift+drag a sound clip", "let it overlap its neighbour (a crossfade)"),
            ("drag a cut's sound edge", "its sound alone past the picture or short of it: J / L cut (the cut selected; it snaps to joins and the sounds beside, Alt: free)"),
            ("drag a fade knob", "fade in / out"),
            ("Ctrl+click a sound", "a volume point there"),
            ("Ctrl+click a point", "take it away"),
            ("drag a level tag", "the clip's own level; double-click: back to normal"),
            ("right-click", "the sound or overlay menu"),
            ("drag a diamond or mark", "move the keyframe in time"),
            ("click / drag a marker's flag", "select it, go to it / move it (the row over the ruler)"),
            ("double- or right-click a flag", "name the marker, or delete it"),
            ("N", "snapping on / off (a yellow line where a drag lands)"),
            ("Alt while dragging", "this drag free, no snapping"),
            ("scroll  /  Shift+scroll", "zoom the timeline / pan it"),
            ("Ctrl+= / Ctrl+− / Ctrl+0", "zoom in / out / the whole edit"),
        ],
    ),
    (
        "Sound",
        &[
            ("K", "a volume point at the playhead on the selected sound"),
            ("S / M on a lane", "solo / mute it"),
        ],
    ),
    (
        "Framing (the Framing switch on)",
        &[
            ("drag the picture", "look around"),
            ("Shift+← / →, ↑ / ↓", "turn, tilt"),
            ("Q / E or right-drag", "roll"),
            ("scroll, + / −", "zoom"),
            ("0 or Home", "reset the view"),
            ("K", "keyframe the view here"),
            ("[ / ]", "the previous / next keyframe"),
            ("J", "the keyframe's ease"),
            ("Del", "delete the keyframe"),
            ("S", "stabilisation"),
            ("P", "projection"),
            ("R", "rolling shutter"),
        ],
    ),
    (
        "Panels and files",
        &[
            ("G", "effects"),
            ("B", "bin"),
            ("Shift+B", "bin in tiles, in the picture's place"),
            ("I / O", "bin player: in and out of a segment (Alt clears)"),
            ("Delete", "bin player: delete the picked segment"),
            ("Ctrl+,", "settings"),
            ("Ctrl+E", "export"),
            ("Ctrl+N", "new: an edit in a trip, a trip, an edit on its own"),
            ("Ctrl+O", "add clips"),
            ("Ctrl+Shift+O", "open a project"),
            ("Ctrl+S / Ctrl+Shift+S", "save / save as"),
            ("F1", "this list"),
        ],
    ),
];

/// `groups` as the guide's reference page: `title`, a line saying where
/// it comes from (`made_by`), and a table per group.
pub fn markdown(title: &str, made_by: &str, groups: Groups) -> String {
    let cell = |s: &str| s.replace('|', "\\|");
    let mut out = format!("# {title}\n\n*{made_by}: the same list as the program's own (F1). Don't edit it by hand.*\n");
    for (group, lines) in groups {
        out += &format!("\n## {group}\n\n| Key | Does |\n|---|---|\n");
        for (key, does) in lines.iter() {
            out += &format!("| {} | {} |\n", cell(key), cell(does));
        }
    }
    out
}

/// Omnix's page.
pub fn omnix_page() -> String {
    markdown("Keys: Omnix", "Made by `omnix keys`", GROUPS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_group_is_a_table_and_a_bar_in_a_key_is_kept() {
        let page = markdown("Keys", "Made by a test", &[("Play", &[("Space", "play / pause"), ("a | b", "both")])]);
        assert!(page.starts_with("# Keys\n\n*Made by a test"));
        assert!(page.contains("## Play\n\n| Key | Does |\n|---|---|\n| Space | play / pause |\n| a \\| b | both |\n"), "{page}");
    }

    /// The guide's reference page is the program's own list, as it is
    /// now: `OMNIX_BLESS=1 cargo test` writes it afresh.
    #[test]
    fn the_guides_keys_page_is_the_programs_list() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/guide/keys-omnix.md");
        let page = omnix_page();
        if std::env::var_os("OMNIX_BLESS").is_some() {
            std::fs::write(&path, &page).unwrap();
        }
        assert_eq!(std::fs::read_to_string(&path).unwrap_or_default(), page, "{} is behind the keys: OMNIX_BLESS=1 cargo test writes it", path.display());
    }
}
