//! The `omnix` command: the command line, and the player (`play`). All of
//! it lives in the library (`cli`).

fn main() -> anyhow::Result<()> {
    omnix::cli::main()
}
