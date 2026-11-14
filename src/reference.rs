//! A program's command line as the guide's reference page (`omnix
//! commands`, `dailix commands`): every command with what it does and
//! its arguments, made from the same definitions `--help` prints, so the
//! page can't drift from the program (one fact, one place; a test holds
//! `docs/guide/commands-omnix.md` to it).

use clap::Command;

/// `cmd`'s commands as Markdown, as typed after `bin`: `title`, where it
/// comes from (`made_by`), then a section per command (nested ones too),
/// each with its description and a table of its arguments.
pub fn commands(cmd: &Command, bin: &str, title: &str, made_by: &str) -> String {
    let mut out = format!("# {title}\n\n*{made_by}: the same text as `--help`. Don't edit it by hand.*\n");
    if let Some(about) = cmd.get_about() {
        out += &format!("\n{about}\n");
    }
    for sub in cmd.get_subcommands().filter(|s| s.get_name() != "help") {
        section(&mut out, sub, bin);
    }
    out
}

/// One command's section, then its own commands'.
fn section(out: &mut String, cmd: &Command, parent: &str) {
    let path = format!("{parent} {}", cmd.get_name());
    *out += &format!("\n## `{path}`\n");
    if let Some(about) = cmd.get_long_about().or(cmd.get_about()) {
        *out += &format!("\n{}\n", untagged(about.to_string().trim()));
    }
    let args: Vec<String> = cmd.get_arguments().filter(|a| !matches!(a.get_id().as_str(), "help" | "version") && !a.is_global_set()).map(row).collect();
    if !args.is_empty() {
        *out += "\n| Argument | What it is |\n|---|---|\n";
        for a in args {
            *out += &a;
        }
    }
    for sub in cmd.get_subcommands().filter(|s| s.get_name() != "help") {
        section(out, sub, &path);
    }
}

/// An argument's table row: how it's written, and its help (its default
/// and its choices said after).
fn row(a: &clap::Arg) -> String {
    let value = a.get_value_names().map(|v| v.iter().map(|n| format!("<{n}>")).collect::<Vec<_>>().join(" ")).unwrap_or_else(|| format!("<{}>", a.get_id().as_str().to_uppercase()));
    // A flag (`--dry-run`) takes no value; asked before the command is
    // built, its number of values isn't known, its action is.
    let takes = a.get_action().takes_values();
    let name = match (a.get_short(), a.get_long()) {
        (_, None) => value,
        (s, Some(l)) => {
            let short = s.map_or(String::new(), |s| format!("-{s}, "));
            if takes { format!("{short}--{l} {value}") } else { format!("{short}--{l}") }
        }
    };
    let mut help = a.get_long_help().or(a.get_help()).map(|h| h.to_string().split_whitespace().collect::<Vec<_>>().join(" ")).unwrap_or_default();
    let defaults: Vec<String> = a.get_default_values().iter().map(|v| v.to_string_lossy().into_owned()).filter(|v| !v.is_empty()).collect();
    if !defaults.is_empty() && takes {
        help += &format!(" (default: {})", defaults.join(", "));
    }
    let choices: Vec<String> = a.get_possible_values().iter().filter(|p| !p.is_hide_set()).map(|p| p.get_name().to_owned()).collect();
    if !choices.is_empty() && takes {
        help += &format!(" (one of: {})", choices.join(", "));
    }
    let cell = |s: &str| s.replace('|', "\\|");
    format!("| `{}` | {} |\n", cell(&name), untagged(&cell(help.trim())))
}

/// `<DIR>` in a help text would be read as an HTML tag and vanish: `<`
/// escaped, but not inside a `code span`, where Markdown shows `&lt;` as
/// it is (the audit, 2026-11-14).
fn untagged(text: &str) -> String {
    text.split('`').enumerate().map(|(i, piece)| if i % 2 == 0 { piece.replace('<', "&lt;") } else { piece.to_owned() }).collect::<Vec<_>>().join("`")
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Arg, ArgAction};

    #[test]
    fn a_command_is_a_section_with_its_arguments() {
        let cmd = Command::new("tool").about("A tool").subcommand(
            Command::new("cut")
                .about("Cut a | b")
                .arg(Arg::new("input").required(true).help("The clip"))
                .arg(Arg::new("res").long("resolution").short('r').default_value("1080p").help("How big"))
                .arg(Arg::new("dry").long("dry-run").action(ArgAction::SetTrue).help("Say, don't do"))
                .subcommand(Command::new("more").about("Deeper")),
        );
        let page = commands(&cmd, "tool", "Commands: tool", "Made by a test");
        assert!(page.contains("\n## `tool cut`\n\nCut a | b\n"), "{page}");
        assert!(page.contains("| `<INPUT>` | The clip |\n"), "{page}");
        assert!(page.contains("| `-r, --resolution <RES>` | How big (default: 1080p) |\n"), "{page}");
        assert!(page.contains("| `--dry-run` | Say, don't do |\n"), "{page}");
        assert!(page.contains("\n## `tool cut more`\n\nDeeper\n"), "{page}");
        // A <WORD> in a help text kept from being read as an HTML tag.
        let tagged = Command::new("t").subcommand(Command::new("go").about("Into <DIR>").arg(Arg::new("x").long("x").help("A <FILE>")));
        let page = commands(&tagged, "t", "T", "Made by a test");
        assert!(page.contains("Into &lt;DIR>") && page.contains("| A &lt;FILE> |"), "{page}");
        assert_eq!(untagged("clip `<day>/<file>` holds <speech>"), "clip `<day>/<file>` holds &lt;speech>");
    }

    /// The guide's page is Omnix's command line as it is now:
    /// `OMNIX_BLESS=1 cargo test` writes it afresh.
    #[test]
    fn the_guides_commands_page_is_the_programs() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/guide/commands-omnix.md");
        let page = crate::cli::commands_page();
        if std::env::var_os("OMNIX_BLESS").is_some() {
            std::fs::write(&path, &page).unwrap();
        }
        assert_eq!(std::fs::read_to_string(&path).unwrap_or_default(), page, "{} is behind the commands: OMNIX_BLESS=1 cargo test writes it", path.display());
    }
}
