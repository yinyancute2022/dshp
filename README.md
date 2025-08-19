# dshp — Dead-simple HTTP proxy

**IMPORTANT:** This project and its code were generated entirely by an AI assistant. The implementation may contain bugs, security issues, missing features, or inaccuracies. Review, test, and harden the code before using it in any production or public-facing environment.

A minimal learning-focused HTTP proxy written in Rust using tokio + hyper.
It supports forwarding plain HTTP requests and tunneling HTTPS via the CONNECT method.
This repo includes a small CLI, optional Basic proxy auth, debug logging, a musl-based Dockerfile
for small images, and a GitHub Actions workflow for multi-platform builds.

## Features

- Forward plain HTTP requests using `hyper::Client`
- Support `CONNECT` with proper tunnel splicing (hyper upgrade + tokio copy_bidirectional)
- Optional Basic Proxy-Authorization (--username / --password)
- CLI: configure listen address, auth, and debug logging
- Multi-platform Dockerfile (musl static binary + Alpine runtime) for small images

## Build (local)

Requires Rust toolchain (recommended 1.70+ / rustup):

```bash
cargo build --release
```

Binary will be at `target/release/dshp` (or `target/<target>/release/dshp` when cross-building).

## Run (local)

Default listen address is `0.0.0.0:8080`.

```bash
# default (no auth, debug off)
cargo run --release

# specify listen address, enable debug logs
cargo run --release -- --listen 0.0.0.0:8080 --debug

# require proxy auth
cargo run --release -- --username alice --password secret
```

CLI flags

- `--listen` — address:port to bind (default: `0.0.0.0:8080`)
- `--username` — enable Basic proxy auth when non-empty
- `--password` — proxy password (used only if username is set)
- `--debug` — enable simple debug logs (printed to stderr)

## Examples

HTTP request via proxy (no auth):

```bash
curl -x http://127.0.0.1:8080 http://example.com/
```

HTTPS request via proxy (CONNECT tunnel):

```bash
curl -x http://127.0.0.1:8080 https://www.example.com/
```

If proxy auth is enabled (`--username alice --password secret`), provide credentials with curl:

```bash
curl -x http://127.0.0.1:8080 -U alice:secret https://www.example.com/
# or add header:
# curl -x http://127.0.0.1:8080 -H "Proxy-Authorization: Basic $(echo -n 'alice:secret' | base64)" https://...
```

## Docker (small images)

A multi-stage `Dockerfile` is included in this repo and prebuilt multi-arch images are published to GitHub Container Registry.

You can pull the latest published image with:

```bash
docker pull ghcr.io/yinyancute2022/dshp:latest
```

The published image supports linux/amd64, linux/arm64 and linux/arm/v7 (armv7l).



## Limitations & Security

- This project is intended for learning and experimentation, not production use.
- Header handling is minimal. A production proxy must strip hop-by-hop headers and carefully manage connection headers.
- Authentication is Basic and should only be used over trusted networks or with additional transport security.
- Binding to `0.0.0.0` exposes the proxy to the network. Use firewall rules or bind to localhost if you don't want it public.

## Contributing / Next steps

If you'd like, I can:
- Further reduce image size (copy into `scratch` and bake in certs) — requires ensuring fully static binary and cert handling;
- Add structured logging (tracing) with levels instead of `eprintln!`;
- Improve header hygiene (remove hop-by-hop headers, manage Connection/Keep-Alive);
- Add tests (integration testing for CONNECT and HTTP forwarding).

---

Licensed for personal experimentation. Adjust and harden before exposing to untrusted networks.

