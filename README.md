<div align="center">

<img src="https://capsule-render.vercel.app/api?type=waving&color=0:0f172a,50:38bdf8,100:a78bfa&height=160&section=header&text=ProxyLog&fontSize=48&fontColor=ffffff&fontAlignY=38&animation=fadeIn" alt="header" />

</div>

<div align="center">

# ProxyLog

**A lightweight HTTP proxy for intercepting, inspecting, and logging HTTP traffic.**

[![Rust](https://img.shields.io/badge/Rust-2021-orange?logo=rust)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey)]()
[![CI](https://github.com/yourusername/ProxyLog/actions/workflows/ci.yml/badge.svg)](https://github.com/yourusername/ProxyLog/actions)

```
 ____                              _____ _
|  _ \ ___  _ __ ___   __ _ _ __  |  ___(_)_ __   __ _
| |_) / _ \| '_ ` _ \ / _` | '__| | |_  | | '_ \ / _` |
|  __/ (_) | | | | | | (_| | |    |  _| | | | | | (_| |
|_|   \___/|_| |_| |_|\__,_|_|    |_|   |_|_| |_|\__, |
                                                  |___/
```

</div>

---

## Features

- **HTTP Request/Response Logging** — See every request that passes through the proxy
- **Color-Coded Status Codes** — Green for 2xx, yellow for redirects, red for errors
- **Verbose Mode** — Inspect headers, body previews, and request/response details
- **Request Body Logging** — Capture and preview POST/PUT/PATCH payloads
- **Log to File** — Persist all logs to a file instead of stdout
- **Connection Counting** — Track connections with sequential IDs
- **Configurable Bind** — Choose your host and port
- **Zero Dependencies at Runtime** — Statically linked, single binary output

## Architecture

```
┌──────────┐       HTTP GET /index.html       ┌────────────┐
│          │ ──────────────────────────────▶   │            │
│  Browser │                                   │  ProxyLog  │
│          │ ◀──────────────────────────────   │            │
└──────────┘       200 OK + HTML body          └─────┬──────┘
                                                     │
                                               ┌─────▼──────┐
                                               │            │
                                               │  Upstream  │
                                               │   Server   │
                                               │            │
                                               └────────────┘
```

**How it works:**

1. ProxyLog binds to a local address and listens for incoming HTTP connections
2. Your browser (or any HTTP client) is configured to use it as an HTTP proxy
3. When a request arrives, ProxyLog logs the method, URI, status code, and timing
4. In verbose mode, it also logs all headers and a preview of request/response bodies
5. The request is forwarded to the actual destination server
6. The response is returned to the client with an `X-Proxy-Log: true` header injected
7. In verbose mode, a preview of the response body is also logged

## CLI Options

| Flag | Long | Default | Description |
|------|------|---------|-------------|
| `-p` | `--port` | `8080` | Port to listen on |
| `-H` | `--host` | `127.0.0.1` | Host to bind to |
| `-v` | `--verbose` | `false` | Enable verbose output (headers, body preview) |
| | `--log-file` | *(stdout)* | Path to log file for persistent logging |
| | `--max-body-log` | `512` | Max bytes to log from request/response bodies |

## Quick Start

### Build from Source

```bash
# Clone the repository
git clone https://github.com/yourusername/ProxyLog.git
cd ProxyLog

# Build in release mode
cargo build --release

# The binary is now at:
# ./target/release/proxylog   (Linux/macOS)
# ./target/release/proxylog.exe   (Windows)
```

### Run

```bash
# Start proxy on default port 8080
./target/release/proxylog

# Custom port with verbose output
./target/release/proxylog --port 9090 --verbose

# Log everything to a file
./target/release/proxylog --verbose --log-file proxy.log

# Bind to all interfaces (for remote access)
./target/release/proxylog --host 0.0.0.0 --port 8080
```

## Example Output

**Normal mode:**
```
[14:32:01.123]    200     GET http://example.com/
  └─ 45ms
[14:32:01.456]    304     GET http://example.com/style.css
  └─ 12ms
[14:32:02.789]    404     GET http://example.com/missing
  └─ 8ms
```

**Verbose mode:**
```
[14:32:01.123]    200     GET http://example.com/
  └─ 45ms
-> GET http://example.com/ HTTP/1.1
   Host: example.com
   User-Agent: Mozilla/5.0
   Accept: text/html
   [body] {"query":"search"} (14 bytes)
<- [response body] <!DOCTYPE html>... (1256 bytes)
```

## Browser Proxy Configuration

### Firefox

1. Open **Settings** → **General** → **Network Settings**
2. Select **Manual proxy configuration**
3. Set **HTTP Proxy** to `127.0.0.1`, Port to `8080`
4. Check **Also use this proxy for HTTPS**
5. Click **OK**

### Chrome / Edge

1. Open **Settings** → **System** → **Open proxy settings**
2. Under **Manual proxy setup**, click **Use a proxy server**
3. Enter address `127.0.0.1` and port `8080`
4. Click **Save**

### System-wide (Linux)

```bash
export HTTP_PROXY=http://127.0.0.1:8080
export HTTPS_PROXY=http://127.0.0.1:8080

# Now curl, wget, etc. will use the proxy
curl http://example.com
```

## Programmatic Usage

ProxyLog can be embedded in other Rust projects:

```rust
use std::net::SocketAddr;
use tokio::net::TcpListener;

// The proxy logic can be integrated into any async Rust application
// by using the hyper and tokio crates directly.
//
// Key components:
//   - hyper::server::conn::http1 for handling connections
//   - hyper::service::service_fn for request routing
//   - hyper_util::client::legacy::Client for upstream requests
//
// See src/main.rs for a complete working example.
```

## Project Structure

```
ProxyLog/
├── Cargo.toml              # Package manifest and dependencies
├── README.md               # This file
├── LICENSE                  # MIT License
├── .gitignore              # Git ignore rules
├── .github/
│   └── workflows/
│       └── ci.yml          # GitHub Actions CI pipeline
├── src/
│   └── main.rs             # Application entry point and proxy logic
└── tests/
    └── integration_test.rs # Integration tests
```

## Testing

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run a specific test
cargo test test_proxy_starts
```

## Limitations

- **No HTTPS/CONNECT support** — This is a logging proxy; HTTPS tunneling (CONNECT method) returns `501 Not Implemented`. For HTTPS interception, consider tools like mitmproxy.
- **HTTP/1.1 only** — Does not support HTTP/2 or HTTP/3
- **Single-threaded forwarding** — Each connection is handled independently; no connection pooling
- **No authentication** — Anyone who can reach the proxy port can use it
- **No request modification** — ProxyLog is read-only; it logs but does not alter requests

## License

This project is licensed under the [MIT License](LICENSE).
