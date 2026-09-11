use bytes::Bytes;
use chrono::Local;
use clap::Parser;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::fs::OpenOptions;
use std::io::Write;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::net::TcpListener;

static CONNECTION_COUNT: AtomicU64 = AtomicU64::new(0);
static REQUEST_COUNT: AtomicU64 = AtomicU64::new(0);

#[derive(Parser)]
#[command(
    name = "proxylog",
    about = "Mini HTTP proxy with request logging",
    version,
    long_about = "A lightweight HTTP proxy server for intercepting, inspecting, and logging HTTP traffic.\nUseful for debugging, development, and learning how HTTP works."
)]
struct Cli {
    #[arg(short, long, default_value = "8080", help = "Port to listen on")]
    port: u16,

    #[arg(short, long, default_value = "127.0.0.1", help = "Host to bind to")]
    host: String,

    #[arg(short, long, help = "Enable verbose output (headers, body preview)")]
    verbose: bool,

    #[arg(long, help = "Path to log file (logs to stdout if not set)")]
    log_file: Option<PathBuf>,

    #[arg(long, help = "Max body bytes to log in verbose mode", default_value = "512")]
    max_body_log: usize,
}

struct LogWriter {
    file: Option<std::sync::Mutex<std::fs::File>>,
}

impl LogWriter {
    fn new(path: &Option<PathBuf>) -> Self {
        let file = path.as_ref().map(|p| {
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(p)
                .unwrap_or_else(|e| {
                    eprintln!("Failed to open log file {:?}: {}", p, e);
                    std::process::exit(1);
                })
        });
        Self {
            file: file.map(std::sync::Mutex::new),
        }
    }

    fn write_log(&self, msg: &str) {
        if let Some(ref file) = self.file {
            if let Ok(mut f) = file.lock() {
                let _ = writeln!(f, "{}", msg);
            }
        } else {
            println!("{}", msg);
        }
    }
}

fn log_request(
    method: &Method,
    uri: &str,
    status: StatusCode,
    duration_ms: u128,
    writer: &LogWriter,
    conn_id: u64,
) {
    let time = Local::now().format("%H:%M:%S%.3f");
    let status_color = if status.is_success() {
        "\x1b[32m"
    } else if status.is_client_error() || status.is_server_error() {
        "\x1b[31m"
    } else if status.is_redirection() {
        "\x1b[33m"
    } else {
        "\x1b[36m"
    };
    let msg = format!(
        "[{}] #{:<5} {} {:>7} {:>7} {} {}{}",
        time,
        conn_id,
        status_color,
        status.as_u16(),
        method,
        uri,
        "\x1b[0m",
    );
    writer.write_log(&msg);
    if duration_ms > 0 {
        writer.write_log(&format!("  \u{2514}\u{2500} {}ms", duration_ms));
    }
}

fn truncate_body(data: &[u8], max: usize) -> String {
    if data.len() <= max {
        String::from_utf8_lossy(data).to_string()
    } else {
        format!(
            "{}... ({} bytes truncated)",
            String::from_utf8_lossy(&data[..max]),
            data.len() - max
        )
    }
}

async fn handle_request(
    req: Request<Incoming>,
    verbose: bool,
    max_body_log: usize,
    writer: Arc<LogWriter>,
    conn_id: u64,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let uri_str = uri.to_string();

    let start = std::time::Instant::now();

    if verbose {
        writer.write_log(&format!(
            "\x1b[90m-> {} {} {:?}\x1b[0m",
            method,
            uri_str,
            req.version()
        ));
        for (key, value) in req.headers() {
            writer.write_log(&format!(
                "   \x1b[90m{}: {}\x1b[0m",
                key,
                value.to_str().unwrap_or("<binary>")
            ));
        }
    }

    let response = match method {
        Method::CONNECT => {
            log_request(
                &method,
                &uri_str,
                StatusCode::NOT_IMPLEMENTED,
                0,
                &writer,
                conn_id,
            );
            Response::builder()
                .status(StatusCode::NOT_IMPLEMENTED)
                .body(Full::new(Bytes::from(
                    "CONNECT not supported in logging mode",
                )))
                .unwrap()
        }
        _ => {
            let has_body = matches!(
                method,
                Method::POST | Method::PUT | Method::PATCH | Method::DELETE
            );

            let (parts, body) = req.into_parts();
            let collected = body.collect().await?.to_bytes();

            if has_body && verbose {
                let preview = truncate_body(&collected, max_body_log);
                writer.write_log(&format!(
                    "\x1b[90m   [body] {} ({} bytes)\x1b[0m",
                    preview,
                    collected.len()
                ));
            }

            let client = hyper_util::client::legacy::Client::new(hyper_util::rt::TokioExecutor::new());
            let req_builder = Request::builder()
                .method(method.clone())
                .uri(uri.clone())
                .body(Full::new(collected))
                .unwrap();

            match client.request(req_builder).await {
                Ok(resp) => {
                    let status = resp.status();
                    let elapsed = start.elapsed().as_millis();
                    log_request(&method, &uri_str, status, elapsed, &writer, conn_id);

                    let (parts, body) = resp.into_parts();
                    let bytes = body.collect().await?.to_bytes();

                    if verbose {
                        let preview = truncate_body(&bytes, max_body_log);
                        writer.write_log(&format!(
                            "\x1b[90m<- [response body] {} ({} bytes)\x1b[0m",
                            preview,
                            bytes.len()
                        ));
                    }

                    let mut response = Response::from_parts(parts, Full::new(bytes));
                    response
                        .headers_mut()
                        .insert("X-Proxy-Log", "true".parse().unwrap());
                    response
                }
                Err(e) => {
                    let elapsed = start.elapsed().as_millis();
                    writer.write_log(&format!(
                        "\x1b[31mError proxying {}: {}\x1b[0m",
                        uri_str, e
                    ));
                    log_request(
                        &method,
                        &uri_str,
                        StatusCode::BAD_GATEWAY,
                        elapsed,
                        &writer,
                        conn_id,
                    );
                    Response::builder()
                        .status(StatusCode::BAD_GATEWAY)
                        .body(Full::new(Bytes::from(format!("Proxy error: {}", e))))
                        .unwrap()
                }
            }
        }
    };

    Ok(response)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let addr: SocketAddr = format!("{}:{}", cli.host, cli.port).parse()?;

    let writer = Arc::new(LogWriter::new(&cli.log_file));

    let listener = TcpListener::bind(addr).await?;

    let banner = format!(
        r#"
\x1b[36m ____                              _____ _
\x1b[36m|  _ \ ___  _ __ ___   __ _ _ __  |  ___(_)_ __   __ _
\x1b[36m| |_) / _ \| '_ ` _ \ / _` | '__| | |_  | | '_ \ / _` |
\x1b[36m|  __/ (_) | | | | | | (_| | |    |  _| | | | | | (_| |
\x1b[36m|_|   \___/|_| |_| |_|\__,_|_|    |_|   |_|_| |_|\__, |
\x1b[36m                                                  |___/\x1b[0m

\x1b[32mListening on\x1b[0m    {}
\x1b[32mVerbose\x1b[0m         {}
\x1b[32mLog file\x1b[0m        {}
\x1b[32mMax body log\x1b[0m   {} bytes

Configure your browser to use \x1b[1m{}:{}\x1b[0m as HTTP proxy.
"#,
        addr,
        if cli.verbose { "ON" } else { "OFF" },
        cli.log_file
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "stdout".to_string()),
        cli.max_body_log,
        cli.host,
        cli.port,
    );
    println!("{}", banner);

    loop {
        let (stream, remote) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let verbose = cli.verbose;
        let writer = Arc::clone(&writer);
        let max_body_log = cli.max_body_log;

        let conn_id = CONNECTION_COUNT.fetch_add(1, Ordering::Relaxed);

        tokio::spawn(async move {
            let service = service_fn(move |req| {
                let verbose = verbose;
                let writer = Arc::clone(&writer);
                let max_body_log = max_body_log;
                let conn_id = conn_id;
                async move {
                    REQUEST_COUNT.fetch_add(1, Ordering::Relaxed);
                    handle_request(req, verbose, max_body_log, writer, conn_id).await
                }
            });

            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service)
                .await
            {
                writer.write_log(&format!(
                    "\x1b[31mConnection error from {}: {}\x1b[0m",
                    remote, err
                ));
            }
        });
    }
}
