use std::{io, path::PathBuf};

use thiserror::Error;

pub type BaoResult<T> = Result<T, BaoError>;

#[derive(Debug, Error)]
pub enum BaoError {
    #[error("no configuration directory for this user")]
    NoConfigDirectory,
    #[error("no OpenBao address: run `baoenv setup`, pass --addr or set BAO_ADDR")]
    NoAddress,
    #[error("{0} is not set")]
    MissingVariable(&'static str),
    #[error(
        "no path: pass --environment and --service, or run inside a directory that baoenv.toml maps"
    )]
    NoTarget,
    #[error("no identity: set BAO_TOKEN, or BAO_ROLE_ID and BAO_SECRET_ID for an AppRole")]
    NoIdentity,
    #[error("cloudflared is not installed; install it and run `cloudflared access login {0}`")]
    CloudflaredMissing(String),
    #[error("no Cloudflare Access session; run `cloudflared access login {0}`")]
    NoAccessSession(String),
    #[error("no clipboard tool worked; install wl-clipboard or xclip, or pipe `baoenv token`")]
    NoClipboard,
    #[error("OpenBao refused the login: {0}")]
    LoginRefused(reqwest::Error),
    #[error("reading {path}: {source}")]
    Read {
        path: String,
        source: reqwest::Error
    },
    #[error("running {0}: {1}")]
    Spawn(String, io::Error),
    #[error("{0}: {1}")]
    File(PathBuf, io::Error),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Header(#[from] reqwest::header::InvalidHeaderValue),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Prompt(#[from] dialoguer::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Toml(#[from] toml::de::Error),
    #[error(transparent)]
    TomlWrite(#[from] toml::ser::Error)
}
