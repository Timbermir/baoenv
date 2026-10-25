use std::{
    env,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output}
};

use httpmock::{Method, MockServer};

struct Sandbox(PathBuf);

impl Sandbox {
    fn new(name: &str) -> Self {
        let path = env::temp_dir().join(format!("baoenv-{}-{name}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn baoenv(&self, server: &MockServer, args: &[&str]) -> Output {
        self.baoenv_in(server, &self.0, args)
    }

    fn baoenv_in(&self, server: &MockServer, directory: &Path, args: &[&str]) -> Output {
        self.command(server, args)
            .current_dir(directory)
            .env("CF_ACCESS_CLIENT_ID", "id")
            .env("CF_ACCESS_CLIENT_SECRET", "secret")
            .env("BAO_ROLE_ID", "role")
            .env("BAO_SECRET_ID", "secret")
            .output()
            .unwrap()
    }

    fn command(&self, server: &MockServer, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_baoenv"));
        command
            .args(args)
            .env_clear()
            .env("PATH", env::var("PATH").unwrap())
            .env("HOME", &self.0)
            .env("XDG_CONFIG_HOME", &self.0)
            .env("BAO_ADDR", server.base_url());
        command
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

#[test]
fn approle_login_reads_secrets_and_caches_the_token() {
    let server = MockServer::start();
    let sandbox = Sandbox::new("env");
    let login = server.mock(|when, then| {
        when.method(Method::POST)
            .path("/v1/auth/approle/login")
            .header("CF-Access-Client-Id", "id")
            .json_body(serde_json::json!({ "role_id": "role", "secret_id": "secret" }));
        then.status(200)
            .json_body(serde_json::json!({ "auth": { "client_token": "t-1" } }));
    });
    let read = server.mock(|when, then| {
        when.method(Method::GET)
            .path("/v1/kv/data/polygon/api")
            .header("X-Vault-Token", "t-1");
        then.status(200).json_body(serde_json::json!({
            "data": { "data": { "A": "1", "B": "it's", "N": 2 } }
        }));
    });

    let output = sandbox.baoenv(&server, &["env", "-e", "polygon", "-s", "api"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "A='1'\nB='it'\\''s'\nN='2'\n"
    );
    login.assert();
    read.assert();
    assert_eq!(
        fs::read_to_string(sandbox.0.join("baoenv").join("credentials")).unwrap(),
        "t-1"
    );
    let token = sandbox.baoenv(&server, &["token"]);
    assert_eq!(String::from_utf8_lossy(&token.stdout), "t-1\n");
    login.assert();
}

#[test]
fn run_passes_secrets_and_exit_code() {
    let server = MockServer::start();
    let sandbox = Sandbox::new("run");
    server.mock(|when, then| {
        when.method(Method::POST).path("/v1/auth/approle/login");
        then.status(200)
            .json_body(serde_json::json!({ "auth": { "client_token": "t-2" } }));
    });
    server.mock(|when, then| {
        when.method(Method::GET).path("/v1/kv/data/local/api");
        then.status(200)
            .json_body(serde_json::json!({ "data": { "data": { "GREETING": "hi" } } }));
    });

    let output = sandbox.baoenv(&server, &[
        "run",
        "-e",
        "local",
        "-s",
        "api",
        "sh",
        "-c",
        "echo $GREETING; exit 3"
    ]);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hi\n");
    assert_eq!(output.status.code(), Some(3));
}

#[test]
fn plain_openbao_uses_the_token_from_the_environment() {
    let server = MockServer::start();
    let sandbox = Sandbox::new("plain");
    fs::create_dir_all(sandbox.0.join("baoenv")).unwrap();
    fs::write(
        sandbox.0.join("baoenv").join("config.toml"),
        "access = false\n"
    )
    .unwrap();
    let read = server.mock(|when, then| {
        when.method(Method::GET)
            .path("/v1/kv/data/local/api")
            .header("X-Vault-Token", "hvs.plain")
            .header_missing("cf-access-token")
            .header_missing("CF-Access-Client-Id");
        then.status(200)
            .json_body(serde_json::json!({ "data": { "data": { "K": "v" } } }));
    });

    let output = sandbox
        .command(&server, &["env", "-e", "local", "-s", "api"])
        .env("BAO_TOKEN", "hvs.plain")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "K='v'\n");
    read.assert();

    let output = sandbox
        .command(&server, &["env", "-e", "local", "-s", "api"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("no identity"));
}

#[test]
fn dotenv_and_powershell_formats_keep_awkward_values() {
    let server = MockServer::start();
    let sandbox = Sandbox::new("formats");
    server.mock(|when, then| {
        when.method(Method::POST).path("/v1/auth/approle/login");
        then.status(200)
            .json_body(serde_json::json!({ "auth": { "client_token": "t-3" } }));
    });
    server.mock(|when, then| {
        when.method(Method::GET).path("/v1/kv/data/local/api");
        then.status(200)
            .json_body(serde_json::json!({ "data": { "data": {
            "CONFIG": { "url": "postgres://u:p@h/db", "pool": 4 },
            "PEM": "line one\nline \"two\" $HOME \\ it's"
        } } }));
    });

    let output = sandbox.baoenv(&server, &[
        "env", "-e", "local", "-s", "api", "--format", "dotenv"
    ]);
    let parsed: Vec<(String, String)> = dotenvy::from_read_iter(&output.stdout[..])
        .map(Result::unwrap)
        .collect();
    assert_eq!(parsed, [
        (
            "CONFIG".to_string(),
            r#"{"pool":4,"url":"postgres://u:p@h/db"}"#.to_string()
        ),
        (
            "PEM".to_string(),
            "line one\nline \"two\" $HOME \\ it's".to_string()
        )
    ]);

    let output = sandbox.baoenv(&server, &[
        "env",
        "-e",
        "local",
        "-s",
        "api",
        "--format",
        "powershell"
    ]);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "$env:CONFIG = '{\"pool\":4,\"url\":\"postgres://u:p@h/db\"}'\n$env:PEM = 'line one\nline \"two\" $HOME \\ it''s'\n"
    );
}

#[test]
fn the_longest_mapped_directory_picks_the_path() {
    let server = MockServer::start();
    let sandbox = Sandbox::new("project");
    let backend = sandbox.0.join("services").join("api").join("backend");
    fs::create_dir_all(&backend).unwrap();
    fs::write(
        sandbox.0.join("baoenv.toml"),
        "[[paths]]\npath = \"services\"\nenvironment = \"local\"\nservice = \"root\"\n\n[[paths]]\npath = \"services/api\"\nenvironment = \"local\"\nservice = \"api\"\n"
    )
    .unwrap();
    server.mock(|when, then| {
        when.method(Method::POST).path("/v1/auth/approle/login");
        then.status(200)
            .json_body(serde_json::json!({ "auth": { "client_token": "t-4" } }));
    });
    let read = server.mock(|when, then| {
        when.method(Method::GET).path("/v1/kv/data/local/api");
        then.status(200)
            .json_body(serde_json::json!({ "data": { "data": { "K": "v" } } }));
    });

    let output = sandbox.baoenv_in(&server, &backend, &["env"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "K='v'\n");
    read.assert();

    fs::write(
        sandbox.0.join("baoenv.toml"),
        "[[paths]]\npath = \"services\"\nenviroment = \"local\"\nservice = \"root\"\n"
    )
    .unwrap();
    let output = sandbox.baoenv_in(&server, &backend, &["env"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("enviroment"));
}
