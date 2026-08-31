use bytes::Bytes;
use chrono::Local;
use clap::Parser;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use tokio::net::TcpListener;

#[derive(Parser)]
#[command(name = "proxylog", about = "Mini HTTP proxy with request logging")]
struct Cli {
    #[arg(short, long, default_value = "8080")]
    port: u16,

    #[arg(short, long, default_value = "127.0.0.1")]
    host: String,

    #[arg(short, long)]
    verbose: bool,
}

fn log_request(method: &Method, uri: &str, status: StatusCode, duration_ms: u128) {
    let time = Local::now().format("%H:%M:%S%.3f");
    let status_color = if status.is_success() {
        "\x1b[32m"
    } else if status.is_client_error() || status.is_server_error() {
        "\x1b[31m"
    } else {
        "\x1b[33m"
    };
    println!(
        "[{}] {} {:>7} {} {}{}",
        time,
        status_color,
        status.as_u16(),
        method,
        uri,
        "\x1b[0m"
    );
    if duration_ms > 0 {
        println!("  └─ {}ms", duration_ms);
    }
}

async fn handle_request(
    req: Request<Incoming>,
    verbose: bool,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let uri_str = uri.to_string();

    let start = std::time::Instant::now();

    if verbose {
        eprintln!("-> {} {} {:?}", method, uri_str, req.version());
        for (key, value) in req.headers() {
            eprintln!("   {}: {}", key, value.to_str().unwrap_or("?"));
        }
    }

    let response = match method {
        Method::CONNECT => {
            log_request(&method, &uri_str, StatusCode::NOT_IMPLEMENTED, 0);
            Response::builder()
                .status(StatusCode::NOT_IMPLEMENTED)
                .body(Full::new(Bytes::from("CONNECT not supported in logging mode")))
                .unwrap()
        }
        _ => {
            let client = hyper_util::client::legacy::Client::new(
                hyper_util::rt::TokioExecutor::new(),
            );
            let req_builder = Request::builder()
                .method(method.clone())
                .uri(uri.clone())
                .body(Full::new(Bytes::new()))
                .unwrap();

            match client.request(req_builder).await {
                Ok(resp) => {
                    let status = resp.status();
                    let elapsed = start.elapsed().as_millis();
                    log_request(&method, &uri_str, status, elapsed);

                    let (parts, body) = resp.into_parts();
                    let bytes = body.collect().await?.to_bytes();
                    let mut response = Response::from_parts(parts, Full::new(bytes));

                    response.headers_mut().insert(
                        "X-Proxy-Log",
                        "true".parse().unwrap(),
                    );
                    response
                }
                Err(e) => {
                    let elapsed = start.elapsed().as_millis();
                    eprintln!("Error proxying {}: {}", uri_str, e);
                    log_request(&method, &uri_str, StatusCode::BAD_GATEWAY, elapsed);
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

    let listener = TcpListener::bind(addr).await?;
    println!("ProxyLog listening on {}", addr);
    println!("Configure your browser to use this as HTTP proxy\n");

    loop {
        let (stream, remote) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let verbose = cli.verbose;

        tokio::spawn(async move {
            let service = service_fn(move |req| {
                let verbose = verbose;
                async move { handle_request(req, verbose).await }
            });

            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service)
                .await
            {
                eprintln!("Error from {}: {}", remote, err);
            }
        });
    }
}
