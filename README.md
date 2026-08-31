# ProxyLog

Mini HTTP proxy for intercepting and logging requests.

## Features

- HTTP request/response logging
- Color-coded status codes
- Request header inspection
- Proxy error handling
- Configurable host and port

## Build & Run

```bash
cargo build --release

# Start proxy on port 8080
./target/release/proxylog

# Custom port with verbose output
./target/release/proxylog --port 9090 --verbose

# Configure browser to use 127.0.0.1:8080 as HTTP proxy
```

## License

MIT
