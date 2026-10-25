use std::{collections::BTreeMap, env, fs};

use reqwest::{
    StatusCode,
    blocking::{Client as Http, Response}
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    access::Access,
    config::{Config, write_private},
    error::{BaoError, BaoResult}
};

const TOKEN_HEADER: &str = "X-Vault-Token";
const DEFAULT_ROLE: &str = "dev";
const DEFAULT_MOUNT: &str = "kv";

#[derive(Deserialize)]
struct Login {
    auth: Auth
}

#[derive(Deserialize)]
struct Auth {
    client_token: String
}

#[derive(Deserialize)]
struct Lookup {
    data: Identity
}

#[derive(Deserialize)]
struct Identity {
    display_name: String,
    policies: Vec<String>,
    expire_time: Option<String>
}

#[derive(Deserialize)]
struct Secret {
    data: Versioned
}

#[derive(Deserialize)]
struct Versioned {
    data: BTreeMap<String, Value>
}

pub struct Client {
    addr: String,
    access: Access,
    http: Http,
    config: Config
}

impl Client {
    pub fn new(addr: Option<String>) -> BaoResult<Self> {
        let config = Config::load()?;
        let addr = addr
            .or_else(|| config.addr.clone())
            .ok_or(BaoError::NoAddress)?;
        let access = Access::detect(&addr, config.access.unwrap_or(true))?;
        let http = Http::builder().default_headers(access.headers()?).build()?;
        Ok(Self {
            addr,
            access,
            http,
            config
        })
    }

    pub fn secrets(&self, environment: &str, service: &str) -> BaoResult<BTreeMap<String, String>> {
        let mount = self.config.mount.as_deref().unwrap_or(DEFAULT_MOUNT);
        let path = format!("v1/{mount}/data/{environment}/{service}");
        let mut response = self.read(&path, &self.token()?)?;
        if response.status() == StatusCode::FORBIDDEN
            && matches!(self.access, Access::Person { .. })
        {
            response = self.read(&path, &self.login()?)?;
        }
        let secret: Secret = response
            .error_for_status()
            .map_err(|source| BaoError::Read {
                path: path.clone(),
                source
            })?
            .json()?;
        Ok(secret
            .data
            .data
            .into_iter()
            .map(|(key, value)| match value {
                Value::String(text) => (key, text),
                other => (key, other.to_string())
            })
            .collect())
    }

    pub fn whoami(&self) -> BaoResult<String> {
        let token = self.token()?;
        let lookup: Lookup = self
            .read("v1/auth/token/lookup-self", &token)?
            .error_for_status()
            .map_err(|source| BaoError::Read {
                path: "v1/auth/token/lookup-self".to_string(),
                source
            })?
            .json()?;
        Ok(format!(
            "{}\npolicies: {}\nexpires: {}",
            lookup.data.display_name,
            lookup.data.policies.join(", "),
            lookup
                .data
                .expire_time
                .unwrap_or_else(|| "never".to_string())
        ))
    }

    pub fn login(&self) -> BaoResult<String> {
        let token = match &self.access {
            Access::Person { token } => {
                let role = self.config.role.as_deref().unwrap_or(DEFAULT_ROLE);
                self.authenticate("v1/auth/jwt/login", json!({ "role": role, "jwt": token }))?
            }
            Access::Machine { .. } | Access::None => {
                let role_id = env::var("BAO_ROLE_ID").map_err(|_| BaoError::NoIdentity)?;
                let secret_id = env::var("BAO_SECRET_ID")
                    .map_err(|_| BaoError::MissingVariable("BAO_SECRET_ID"))?;
                self.authenticate(
                    "v1/auth/approle/login",
                    json!({ "role_id": role_id, "secret_id": secret_id })
                )?
            }
        };
        write_private(&self.config.credentials()?, &token)?;
        Ok(token)
    }

    pub fn token(&self) -> BaoResult<String> {
        if let Ok(token) = env::var("BAO_TOKEN") {
            return Ok(token);
        }
        match fs::read_to_string(self.config.credentials()?) {
            Ok(token) if !token.trim().is_empty() => Ok(token.trim().to_string()),
            _ => self.login()
        }
    }

    fn authenticate(&self, path: &str, body: Value) -> BaoResult<String> {
        let login: Login = self
            .http
            .post(format!("{}/{path}", self.addr))
            .json(&body)
            .send()?
            .error_for_status()
            .map_err(BaoError::LoginRefused)?
            .json()?;
        Ok(login.auth.client_token)
    }

    fn read(&self, path: &str, token: &str) -> BaoResult<Response> {
        Ok(self
            .http
            .get(format!("{}/{path}", self.addr))
            .header(TOKEN_HEADER, token)
            .send()?)
    }
}
