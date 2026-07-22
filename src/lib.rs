pub mod builtins;
pub mod cli;
pub mod external;
pub mod runtime;
pub mod shared;

pub fn main_entry() -> std::process::ExitCode {
    cli::main_entry()
}
