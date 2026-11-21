# ferroKV

[![CI](https://github.com/Ridju/ferroKV/actions/workflows/ci.yml/badge.svg)](https://github.com/Ridju/ferroKV/actions)

**ferroKV** is a **Key-Value store** written from scratch in **Rust**, featuring a custom **RESP** wire protocol implementation, time-based key expiration, **Append-Only File (AOF)** persistence with startup recovery, snapshot support, asynchronous TCP networking with **Tokio**, **Serde** serialization, **Chrono** timestamps, unit/async tests, **GitHub Actions** CI/CD, a **Devcontainer** setup for development, and a multi-stage **Dockerfile** ready for network deployment.

## Features

- **Hand-rolled Rust implementation:** Core storage engine, expiration tracking, RESP encoder/parser, command layer, and AOF recovery pipeline built from scratch.
- **RESP protocol & commands:** Supports RESP frames (`SimpleString`, `Error`, `Integer`, `BulkString`, `Array`, `Null`) and commands including `GET`, `SET` / `PUT`, and `DEL` / `DELETE` (case-insensitive).
- **Time-based expiration:** Tracks key expiration with `chrono` (`DateTime<Utc>`) and a `BinaryHeap`-backed expiry queue (`ExpiryItem`), combining lazy expiration on reads/existence checks with `cleanup_expired()` (safely ignoring overwritten keys).
- **AOF persistence & crash recovery:** Replays persisted RESP frames on startup via batch execution and streams write commands asynchronously over a `tokio::sync::mpsc` channel to an AOF writer task.
- **Snapshot support:** Serializes and restores store state with `serde` (`save_snapshot` / `load_snapshot`), rebuilding the expiration heap for non-expired entries on load.
- **Async networking with Tokio:** Handles concurrent TCP connections with `tokio::net::TcpListener`, dropping lock guards before `.await` points.
- **Devcontainer, tests & CI/CD:** Ready to develop inside a Devcontainer, includes unit and async integration tests (`cargo test`), and runs automated workflows via **GitHub Actions**.
- **Multi-stage Docker image:** Compiles a release binary in `rust:latest` and packages it into a lean `debian:bookworm-slim` runtime image exposing port `6379`.

## Architecture Overview

- **`net` (`Server`, `Config`, `Connection`):** Accepts TCP connections with Tokio, decodes/encodes RESP frames, executes commands against `KvEngine`, and forwards write commands (`SET`, `PUT`, `DEL`, `DELETE`) to the AOF channel.
- **`resp` (`RespFrame`, `Command`):** Parses and serializes RESP wire frames and maps RESP arrays into typed commands (`Get`, `Put`, `Delete`, `Unknown`).
- **`engine` (`KvEngine`, `AofEngine`):** Translates commands into store operations, supports batch replay for AOF recovery, and persists/loads AOF frames.
- **`memory` (`Store`, `Entry`, `ExpiryItem`):** Stores key-value entries in a `BTreeMap` protected by an `Arc<RwLock<KvState>>`, pairs it with a `BinaryHeap` for expiration ordering, and supports `serde` snapshots.

## Configuration

The binary reads environment variables at startup:

| Variable | Default | Description |
| :--- | :--- | :--- |
| `SERVER_ADDR` | `127.0.0.1:6379` (`0.0.0.0:6379` in Docker) | TCP address and port the server binds to |
| `AOF_PATH` | `data/appendonly.aof` (`/app/data/appendonly.aof` in Docker) | Path to the Append-Only File used for persistence and recovery |

## Getting Started

### Development with Devcontainer (Recommended)
1. Open the repository in your editor/IDE with Devcontainer support (e.g., VS Code) and reopen in the container.
2. Build and run inside the Devcontainer:
   ```bash
   cargo build --release
   cargo run --release
   ```
   *(If you want to connect from outside the container, make sure port `6379` is forwarded in `.devcontainer/devcontainer.json` and bind with `SERVER_ADDR=0.0.0.0:6379`.)*

### Local without Devcontainer
If you run directly on your host machine instead of inside the Devcontainer, you only need Rust and Cargo installed ([rustup.rs](https://rustup.rs/)):
```bash
git clone https://github.com/Ridju/ferroKV.git 
cd ferroKV
cargo run --release
```

### Connecting with `redis-cli`
```bash
redis-cli -h 127.0.0.1 -p 6379
```

Example session:
```text
SET greeting "hello"
GET greeting
DEL greeting
```

## Running with Docker

### Build the image
```bash
docker build -t ferroKV:latest .
```

### Run the container over the network
```bash
docker run -d \
  --name ferroKV \
  -p 6379:6379 \
  -v ferroKV_data:/app/data \
  ferroKV:latest
```

The container defaults to `SERVER_ADDR=0.0.0.0:6379` and `AOF_PATH=/app/data/appendonly.aof`. Mounting a volume at `/app/data` keeps your AOF data across container restarts.

## Testing & CI/CD

Run the test suite (inside the Devcontainer or locally with Cargo):
```bash
cargo test
```

Automated checks and/or builds run via **GitHub Actions** on repository events (such as pushes and pull requests).
