use std::process::ExitCode;

use clap::{
    CommandFactory,
    Parser,
    Subcommand,
    builder::styling::{AnsiColor, Styles}
};

use crate::{
    command::{Env, Login, Run, Setup, Token, Whoami},
    error::BaoResult
};

mod access;
mod client;
mod command;
mod config;
mod error;

const STYLES: Styles = Styles::styled()
    .header(AnsiColor::BrightGreen.on_default().bold())
    .usage(AnsiColor::BrightGreen.on_default().bold())
    .literal(AnsiColor::BrightCyan.on_default().bold())
    .placeholder(AnsiColor::Cyan.on_default())
    .error(AnsiColor::BrightRed.on_default().bold())
    .valid(AnsiColor::BrightCyan.on_default().bold())
    .invalid(AnsiColor::BrightYellow.on_default().bold());

#[derive(Parser)]
#[command(version, about, styles = STYLES)]
struct Cli {
    #[arg(
        long,
        env = "BAO_ADDR",
        help = "OpenBao address; asked for and remembered when omitted"
    )]
    addr: Option<String>,
    #[command(subcommand)]
    action: Option<Action>
}

#[derive(Subcommand)]
enum Action {
    Setup(Setup),
    Login(Login),
    Whoami(Whoami),
    Token(Token),
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
    let Some(action) = cli.action else {
        Cli::command().print_help()?;
        return Ok(ExitCode::SUCCESS);
    };
    match action {
        Action::Setup(setup) => setup.execute(cli.addr),
        Action::Login(login) => login.execute(cli.addr),
        Action::Whoami(whoami) => whoami.execute(cli.addr),
        Action::Token(token) => token.execute(cli.addr),
        Action::Env(env) => env.execute(cli.addr),
        Action::Run(run) => run.execute(cli.addr)
    }
}
