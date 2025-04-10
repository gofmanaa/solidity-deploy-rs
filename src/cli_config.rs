use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[clap(about, author, version)]
pub struct Config {
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Parser, Clone, Debug)]
pub enum Command {
    #[clap()]
    Deploy(DeployConfig),
}

#[derive(Parser, Debug, Clone)]
#[clap(author, version, about, long_about = None)]
pub struct DeployConfig {
    #[clap(long, value_parser, env = "MNEMONIC")]
    pub mnemonic: String,

    #[clap(long, value_parser, env = "CONTRACT_NAME")]
    pub contract_name: String,
}

pub fn build_config() -> Config {
    Config::parse()
}
