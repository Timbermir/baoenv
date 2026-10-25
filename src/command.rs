use std::{
    io::{self, IsTerminal, Write},
    process::{Command, ExitCode, Stdio}
};

use clap::{Args, ValueEnum};
use dialoguer::{Input, Select, theme::ColorfulTheme};

use crate::{
    client::Client,
    config::{Config, Project},
    error::{BaoError, BaoResult}
};

#[cfg(all(unix, not(target_os = "macos")))]
const CLIPBOARD: &[(&str, &[&str])] = &[
    ("wl-copy", &["--sensitive"]),
    ("xclip", &["-selection", "clipboard"])
];
#[cfg(target_os = "macos")]
const CLIPBOARD: &[(&str, &[&str])] = &[("pbcopy", &[])];
#[cfg(windows)]
const CLIPBOARD: &[(&str, &[&str])] = &[("clip", &[])];

#[derive(Args)]
pub struct Target {
    #[arg(
        short,
        long,
        help = "Environment segment of the KV path; from baoenv.toml when omitted"
    )]
    environment: Option<String>,
    #[arg(
        short,
        long,
        help = "Service segment of the KV path; from baoenv.toml when omitted"
    )]
    service: Option<String>
}

impl Target {
    fn resolve(self) -> BaoResult<(String, String)> {
        let mapping = match (&self.environment, &self.service) {
            (Some(_), Some(_)) => None,
            _ => Project::resolve()?
        };
        let environment = self
            .environment
            .or_else(|| mapping.as_ref().map(|mapping| mapping.environment.clone()))
            .ok_or(BaoError::NoTarget)?;
        let service = self
            .service
            .or_else(|| mapping.map(|mapping| mapping.service))
            .ok_or(BaoError::NoTarget)?;
        Ok((environment, service))
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Format {
    Shell,
    Dotenv,
    Powershell,
    Json
}

#[derive(Args)]
#[command(about = "Store the OpenBao address, the login method, the KV mount and your role")]
pub struct Setup;

#[derive(Args)]
#[command(about = "Exchange the Cloudflare Access session for an OpenBao token")]
pub struct Login;

#[derive(Args)]
#[command(about = "Show the identity and policies behind the current token")]
pub struct Whoami;

#[derive(Args)]
#[command(about = "Copy the OpenBao token to the clipboard, or print it when piped")]
pub struct Token;

#[derive(Args)]
#[command(about = "Print the path's secrets")]
pub struct Env {
    #[command(flatten)]
    target: Target,
    #[arg(long, value_enum, default_value_t = Format::Shell, help = "shell: KEY='value'; dotenv: KEY=\"value\"; powershell: $env:KEY = 'value'; json: an object")]
    format: Format
}

#[derive(Args)]
#[command(about = "Run a program with the path's secrets in its environment")]
pub struct Run {
    #[command(flatten)]
    target: Target,
    #[arg(
        long,
        help = "Keep variables already set in the environment over the path's values"
    )]
    preserve_env: bool,
    #[arg(help = "Program to run")]
    program: String,
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        help = "Arguments passed to the program as given"
    )]
    arguments: Vec<String>
}

impl Setup {
    pub fn execute(self, addr: Option<String>) -> BaoResult<ExitCode> {
        let mut config = Config::load()?;
        let theme = ColorfulTheme::default();
        let mut address = Input::<String>::with_theme(&theme).with_prompt("OpenBao address");
        if let Some(current) = addr.or_else(|| config.addr.clone()) {
            address = address.default(current);
        }
        let address = address.interact_text()?;
        let address = address.trim_end_matches('/');
        config.addr = Some(if address.contains("://") {
            address.to_string()
        } else {
            format!("https://{address}")
        });
        config.access = Some(
            Select::with_theme(&theme)
                .with_prompt("Login method")
                .items(["Cloudflare Access", "BAO_TOKEN or AppRole"])
                .default(if config.access.unwrap_or(true) { 0 } else { 1 })
                .interact()?
                == 0
        );
        config.mount = Some(
            Input::with_theme(&theme)
                .with_prompt("KV mount")
                .default(config.mount.take().unwrap_or_else(|| "kv".to_string()))
                .interact_text()?
        );
        config.role = Some(
            Input::with_theme(&theme)
                .with_prompt("OpenBao role")
                .default(config.role.take().unwrap_or_else(|| "dev".to_string()))
                .interact_text()?
        );
        config.save()?;
        Ok(ExitCode::SUCCESS)
    }
}

impl Login {
    pub fn execute(self, addr: Option<String>) -> BaoResult<ExitCode> {
        let client = Client::new(addr)?;
        client.login()?;
        println!("logged in as {}", client.whoami()?);
        Ok(ExitCode::SUCCESS)
    }
}

impl Whoami {
    pub fn execute(self, addr: Option<String>) -> BaoResult<ExitCode> {
        let identity = Client::new(addr)?.whoami()?;
        println!("{identity}");
        Ok(ExitCode::SUCCESS)
    }
}

impl Token {
    pub fn execute(self, addr: Option<String>) -> BaoResult<ExitCode> {
        let token = Client::new(addr)?.token()?;
        if !io::stdout().is_terminal() {
            println!("{token}");
            return Ok(ExitCode::SUCCESS);
        }
        for (program, arguments) in CLIPBOARD {
            let Ok(mut child) = Command::new(program)
                .args(*arguments)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            else {
                continue;
            };
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(token.as_bytes());
            }
            if child.wait()?.success() {
                println!("Token copied to the clipboard");
                return Ok(ExitCode::SUCCESS);
            }
        }
        Err(BaoError::NoClipboard)
    }
}

impl Env {
    pub fn execute(self, addr: Option<String>) -> BaoResult<ExitCode> {
        let (environment, service) = self.target.resolve()?;
        let secrets = Client::new(addr)?.secrets(&environment, &service)?;
        match self.format {
            Format::Shell =>
                for (key, value) in secrets {
                    println!("{key}='{}'", value.replace('\'', "'\\''"));
                },
            Format::Dotenv =>
                for (key, value) in secrets {
                    let value = value
                        .replace('\\', "\\\\")
                        .replace('"', "\\\"")
                        .replace('$', "\\$")
                        .replace('\n', "\\n");
                    println!("{key}=\"{value}\"");
                },
            Format::Powershell =>
                for (key, value) in secrets {
                    println!("$env:{key} = '{}'", value.replace('\'', "''"));
                },
            Format::Json => println!("{}", serde_json::to_string_pretty(&secrets)?)
        }
        Ok(ExitCode::SUCCESS)
    }
}

impl Run {
    pub fn execute(self, addr: Option<String>) -> BaoResult<ExitCode> {
        let (environment, service) = self.target.resolve()?;
        let mut secrets = Client::new(addr)?.secrets(&environment, &service)?;
        if self.preserve_env {
            secrets.retain(|key, _| std::env::var_os(key).is_none());
        }
        let mut command = Command::new(&self.program);
        command.args(&self.arguments).envs(secrets);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            Err(BaoError::Spawn(self.program, command.exec()))
        }
        #[cfg(not(unix))]
        {
            let status = command
                .status()
                .map_err(|error| BaoError::Spawn(self.program.clone(), error))?;
            Ok(status
                .code()
                .and_then(|code| u8::try_from(code).ok())
                .map_or(ExitCode::FAILURE, ExitCode::from))
        }
    }
}
