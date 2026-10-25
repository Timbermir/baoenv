<h1 align="center"><code>baoenv</code></h1>

<div align="center">
  <strong>Run a program with secrets from an OpenBao KV path in its environment</strong>
</div>

<br />

<div align="center">
  <a href="https://crates.io/crates/baoenv">
    <img src="https://img.shields.io/crates/v/baoenv.svg?style=flat-square" alt="crates.io version">
  </a>
  <a href="https://github.com/Timbermir/baoenv/actions/workflows/release.yaml">
    <img src="https://github.com/Timbermir/baoenv/actions/workflows/release.yaml/badge.svg" alt="build status">
  </a>
  <a href="https://crates.io/crates/baoenv">
    <img src="https://img.shields.io/crates/d/baoenv.svg?style=flat-square" alt="crates.io downloads">
  </a>
  <a href="https://github.com/Timbermir/baoenv/releases">
    <img src="https://img.shields.io/github/downloads/Timbermir/baoenv/total.svg?style=flat-square" alt="release downloads">
  </a>
  <a href="LICENSE">
    <img src="https://img.shields.io/crates/l/baoenv.svg?style=flat-square" alt="license">
  </a>
</div>

<p align="center">
  <img width="300" alt="OpenBao mascot" src="https://raw.githubusercontent.com/openbao/artwork/main/color/openbao-color.svg">
</p>

<p align="center"><sub>Mascot from the <a href="https://github.com/openbao/artwork">OpenBao artwork</a> repository, CC BY 4.0.</sub></p>

`baoenv` is what `doppler run` is for Doppler, for an [OpenBao](https://openbao.org) that may sit behind [Cloudflare Access](https://developers.cloudflare.com/cloudflare-one/). It reads one KV v2 path and either prints it or starts a program with it in the environment. Nothing is written to disk except the OpenBao token.

```sh
baoenv setup                                    # address, Access yes/no, KV mount, role, token location
baoenv login                                    # exchanges the Cloudflare Access session for an OpenBao token
baoenv token                                    # copies the token to the clipboard; prints it when piped
baoenv run -- cargo test                        # path from the nearest baoenv.toml
baoenv run -e polygon -s api -- cargo run       # path from flags
baoenv env -e production -s api --format json   # shell (default), dotenv, powershell or json
baoenv run --preserve-env -- make               # keep what the shell already sets
```

## Installation

| Method                                                              | Command                       |
|---------------------------------------------------------------------|-------------------------------|
| [cargo-binstall](https://github.com/cargo-bins/cargo-binstall)      | `cargo binstall baoenv`       |
| cargo                                                               | `cargo install baoenv --locked` |
| [Release archives](https://github.com/Timbermir/baoenv/releases)    | Linux x86_64 and aarch64, macOS aarch64, Windows x86_64 |

---

## Authentication

Humans never hold an OpenBao password. Depending on what sits in front of OpenBao, identity comes from one of three places.

| Setup                             | Where the identity comes from                                                                 |
|-----------------------------------|-----------------------------------------------------------------------------------------------|
| Plain OpenBao                     | `BAO_TOKEN`, or `BAO_ROLE_ID` and `BAO_SECRET_ID` for an AppRole                              |
| Behind Cloudflare Access, people  | `cloudflared access login <addr>` once, then `baoenv login` exchanges the Access token        |
| Behind Cloudflare Access, machines | `CF_ACCESS_CLIENT_ID` and `CF_ACCESS_CLIENT_SECRET` (service token) plus the AppRole pair    |

> [!NOTE]
>
> For a plain OpenBao answer `no` to the Access question in `baoenv setup`; the tool then never calls `cloudflared` and sends no Access headers. A `BAO_TOKEN` in the environment is always used as is.

For people, `baoenv login` sends the Access token to OpenBao's JWT auth method under the role chosen in `baoenv setup`. OpenBao validates it against the Access team's JWKS, so the only credential is the Cloudflare session.

### OpenBao side, once

1. Enable the JWT method and point it at the Access team:

   ```sh
   bao auth enable jwt
   bao write auth/jwt/config jwks_url=https://<team>.cloudflareaccess.com/cdn-cgi/access/certs bound_issuer=https://<team>.cloudflareaccess.com
   bao write auth/jwt/role/dev role_type=jwt bound_audiences=<application aud> user_claim=email bound_claims='{"email": ["dev@example.com"]}' token_policies=dev token_ttl=8h
   ```

2. For machines, an AppRole; the two values it prints go into `BAO_ROLE_ID` and `BAO_SECRET_ID`:

   ```sh
   bao auth enable approle
   bao write auth/approle/role/ci token_policies=ci token_ttl=1h
   bao read -field=role_id auth/approle/role/ci/role-id
   bao write -f -field=secret_id auth/approle/role/ci/secret-id
   ```

---

## `baoenv.toml`

Maps directories of a repository to a KV path, so commands run inside them need no flags. The entry with the longest matching directory wins; `--environment` and `--service` override it.

```toml
[[paths]]
path = "services"
environment = "local"
service = "root"

[[paths]]
path = "services/api"
environment = "local"
service = "api"
```

Secrets are read from `<mount>/data/<environment>/<service>`.
