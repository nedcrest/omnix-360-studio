//! The `rosmo360` command: the command line, and the player (`play`). All of
//! it lives in the library (`cli`).

fn main() -> anyhow::Result<()> {
    rosmo360::cli::main()
}
