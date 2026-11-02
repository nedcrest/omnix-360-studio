//! The core: the edited project and the rules for changing it, with no
//! window, ffmpeg or player in it (`CORE-API.md`, `OPS-API.md`), so the
//! player, the command line, an AI agent and the tests all change a
//! project the same way.

pub mod edit;
pub mod selection;

#[cfg(test)]
mod tests {
    /// The layer's one rule, checked: nothing here reaches the player,
    /// ffmpeg or iced.
    #[test]
    fn the_core_imports_no_player_media_or_window() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/model");
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let text = std::fs::read_to_string(&path).unwrap();
            // Spelt in pieces, or this file would find itself.
            for banned in [["crate::", "player"].concat(), ["crate::", "ffmpeg"].concat(), ["iced", "::"].concat()] {
                assert!(!text.lines().filter(|l| !l.trim_start().starts_with("//")).any(|l| l.contains(&banned)), "{} uses {banned}", path.display());
            }
        }
    }
}
