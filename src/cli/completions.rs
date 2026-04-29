//! Shell completion generation for bash, zsh, fish, powershell.

use clap::CommandFactory;
use clap_complete::{generate, Shell};
use std::io;

/// Generate completions for the given shell and print to stdout.
pub fn run_completions<C: CommandFactory>(shell: Shell) {
    let mut cmd = C::command();
    let name = cmd.get_name().to_string();
    generate(shell, &mut cmd, name, &mut io::stdout());
}
