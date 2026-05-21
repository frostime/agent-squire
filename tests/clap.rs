use clap::CommandFactory;

#[test]
fn clap_definition_is_valid() {
    agent_squire::cli::Cli::command().debug_assert();
}
