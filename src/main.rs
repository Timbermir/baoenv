use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::{
    command::{Env, Login, Run, Setup, Whoami},
    error::BaoResult
};

mod access;
mod client;
mod command;
mod config;
mod error;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[arg(
        long,
        env = "BAO_ADDR",
        help = "OpenBao address; asked for and remembered when omitted"
    )]
    addr: Option<String>,
    #[command(subcommand)]
    action: Action
}

#[derive(Subcommand)]
enum Action {
    Setup(Setup),
    Login(Login),
    Whoami(Whoami),
    Env(Env),
    Run(Run)
}

fn main() -> ExitCode {
    match execute(Cli::parse()) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn execute(cli: Cli) -> BaoResult<ExitCode> {
    match cli.action {
        Action::Setup(setup) => setup.execute(cli.addr),
        Action::Login(login) => login.execute(cli.addr),
        Action::Whoami(whoami) => whoami.execute(cli.addr),
        Action::Env(env) => env.execute(cli.addr),
        Action::Run(run) => run.execute(cli.addr)
    }
}
