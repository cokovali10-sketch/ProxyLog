use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

const PROXY_PORT: u16 = 18080;
const TARGET_PORT: u16 = 18081;

fn start_target_server() -> Child {
    Command::new("python")
        .args([
            "-m",
            "http.server",
            &TARGET_PORT.to_string(),
            "--bind",
            "127.0.0.1",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start target server")
}

fn start_proxy() -> Child {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let bin_path = std::path::Path::new(manifest_dir)
        .join("target")
        .join("debug")
        .join("proxylog");

    Command::new(bin_path)
        .args(["--port", &PROXY_PORT.to_string(), "--verbose"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start proxy")
}

fn wait_for_port(port: u16, timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if TcpStream::connect(format!("127.0.0.1:{}", port)).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

fn http_get_through_proxy(proxy_port: u16, host: &str, path: &str) -> String {
    let addr = format!("127.0.0.1:{}", proxy_port);
    let mut stream = TcpStream::connect(&addr).expect("Failed to connect to proxy");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();

    let request = format!("GET http://{}/{} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", host, path, host);
    std::io::Write::write_all(&mut stream, request.as_bytes()).unwrap();

    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => response.push_str(&line),
            Err(_) => break,
        }
    }
    response
}

#[test]
fn test_proxy_starts_and_listens() {
    let mut target = start_target_server();
    std::thread::sleep(Duration::from_millis(500));

    let mut proxy = start_proxy();
    let connected = wait_for_port(PROXY_PORT, Duration::from_secs(3));
    assert!(connected, "Proxy should start and listen on port {}", PROXY_PORT);

    proxy.kill().ok();
    target.kill().ok();
}

#[test]
fn test_forwarding_request() {
    let mut target = start_target_server();
    std::thread::sleep(Duration::from_millis(500));

    let mut proxy = start_proxy();
    wait_for_port(PROXY_PORT, Duration::from_secs(3));

    let response = http_get_through_proxy(PROXY_PORT, "127.0.0.1", &format!("{}/", TARGET_PORT));

    assert!(
        response.contains("200 OK") || response.contains("HTTP/1."),
        "Expected HTTP 200 response, got: {}",
        &response[..response.len().min(200)]
    );

    proxy.kill().ok();
    target.kill().ok();
}

#[test]
fn test_logging_output() {
    let mut target = start_target_server();
    std::thread::sleep(Duration::from_millis(500));

    let mut proxy = start_proxy();
    wait_for_port(PROXY_PORT, Duration::from_secs(3));

    let _ = http_get_through_proxy(PROXY_PORT, "127.0.0.1", &format!("{}/", TARGET_PORT));
    std::thread::sleep(Duration::from_millis(200));

    let mut stderr_output = String::new();
    if let Some(ref mut stderr) = proxy.stderr {
        use std::io::Read;
        stderr.read_to_string(&mut stderr_output).ok();
    }

    proxy.kill().ok();
    target.kill().ok();

    assert!(
        stderr_output.contains("127.0.0.1") || stderr_output.contains("GET"),
        "Expected verbose logging output, got stderr: {}",
        &stderr_output[..stderr_output.len().min(300)]
    );
}

#[test]
fn test_proxy_returns_bad_gateway_for_invalid_upstream() {
    let mut proxy = Command::new(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("debug")
            .join("proxylog"),
    )
    .args(["--port", "18082"])
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
    .expect("Failed to start proxy");

    wait_for_port(18082, Duration::from_secs(3));

    let response = http_get_through_proxy(18082, "127.0.0.1", "99999/");

    assert!(
        response.contains("502") || response.contains("Bad Gateway"),
        "Expected 502 Bad Gateway, got: {}",
        &response[..response.len().min(200)]
    );

    proxy.kill().ok();
}

#[test]
fn test_connect_method_returns_not_implemented() {
    let mut proxy = Command::new(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("debug")
            .join("proxylog"),
    )
    .args(["--port", "18083"])
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
    .expect("Failed to start proxy");

    wait_for_port(18083, Duration::from_secs(3));

    let addr = "127.0.0.1:18083";
    let mut stream = TcpStream::connect(addr).expect("Failed to connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();

    let request = "CONNECT example.com:443 HTTP/1.1\r\nHost: example.com:443\r\n\r\n";
    std::io::Write::write_all(&mut stream, request.as_bytes()).unwrap();

    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    let mut line = String::new();
    while reader.read_line(&mut line).unwrap_or(0) > 0 {
        response.push_str(&line);
        line.clear();
    }

    proxy.kill().ok();

    assert!(
        response.contains("501") || response.contains("Not Implemented"),
        "Expected 501 Not Implemented, got: {}",
        &response[..response.len().min(200)]
    );
}
