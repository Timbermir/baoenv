use std::{env, process::Command};

use reqwest::header::{HeaderMap, HeaderValue};

use crate::error::{BaoError, BaoResult};

pub enum Access {
    None,
    Person { token: String },
    Machine { id: String, secret: String }
}

impl Access {
    pub fn detect(addr: &str, enabled: bool) -> BaoResult<Self> {
        if !enabled {
            return Ok(Self::None);
        }
        Ok(
            match (
                env::var("CF_ACCESS_CLIENT_ID"),
                env::var("CF_ACCESS_CLIENT_SECRET")
            ) {
                (Ok(id), Ok(secret)) => Self::Machine { id, secret },
                _ => Self::Person {
                    token: token(addr)?
                }
            }
        )
    }

    pub fn headers(&self) -> BaoResult<HeaderMap> {
        let mut headers = HeaderMap::new();
        match self {
            Self::None => {}
            Self::Person { token } => {
                headers.insert("cf-access-token", HeaderValue::from_str(token)?);
            }
            Self::Machine { id, secret } => {
                headers.insert("CF-Access-Client-Id", HeaderValue::from_str(id)?);
                headers.insert("CF-Access-Client-Secret", HeaderValue::from_str(secret)?);
            }
        }
        Ok(headers)
    }
}

fn token(addr: &str) -> BaoResult<String> {
    let output = Command::new("cloudflared")
        .args(["access", "token", &format!("-app={addr}")])
        .output()
        .map_err(|_| BaoError::CloudflaredMissing(addr.to_string()))?;
    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !output.status.success() || token.is_empty() {
        return Err(BaoError::NoAccessSession(addr.to_string()));
    }
    Ok(token)
}
