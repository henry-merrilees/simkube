mod completions;
mod crd;
mod delete;
mod export;
mod run;
mod snapshot;
mod validation;
mod xray;

use clap::{
    crate_version,
    CommandFactory,
    Parser,
    Subcommand,
};
use sk_core::prelude::*;

use crate::validation::ValidateSubcommand;

#[derive(Parser)]
#[command(
    about = "command-line app for running simulations with SimKube",
    version,
    propagate_version = true
)]
struct SkCommandRoot {
    #[command(subcommand)]
    subcommand: SkSubcommand,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
enum SkSubcommand {
    #[command(about = "generate shell completions for skctl")]
    Completions(completions::Args),

    #[command(about = "print SimKube CRDs")]
    Crd,

    #[command(about = "delete a simulation")]
    Delete(delete::Args),

    #[command(about = "export simulation trace data")]
    Export(export::Args),

    #[command(about = "run a simulation")]
    Run(run::Args),

    #[command(about = "take a point-in-time snapshot of a cluster (does not require sk-tracer to be running)")]
    Snapshot(snapshot::Args),

    #[command(subcommand)]
    Validate(ValidateSubcommand),

    #[command(about = "simkube version")]
    Version,

    #[command(about = "explore or prepare trace data for simulation")]
    Xray(xray::Args),
}

#[tokio::main]
async fn main() -> EmptyResult {
    let args = SkCommandRoot::parse();

    match &args.subcommand {
        SkSubcommand::Completions(args) => completions::cmd(args, SkCommandRoot::command()),
        SkSubcommand::Crd => crd::cmd(),
        SkSubcommand::Export(args) => export::cmd(args).await,
        SkSubcommand::Delete(args) => delete::cmd(args).await,
        SkSubcommand::Run(args) => run::cmd(args).await,
        SkSubcommand::Snapshot(args) => snapshot::cmd(args).await,
        SkSubcommand::Validate(subcommand) => validation::cmd(subcommand).await,
        SkSubcommand::Version => {
            println!("skctl {}", crate_version!());
            Ok(())
        },
        SkSubcommand::Xray(args) => xray::cmd(args).await,
    }
}
