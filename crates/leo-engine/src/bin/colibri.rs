use std::{
    env,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::ExitCode,
    thread,
    time::Duration,
};

const DEFAULT_BIND: &str = "0.0.0.0:8080";
const DEFAULT_HEALTH_ADDR: &str = "127.0.0.1:8080";

fn main() -> ExitCode {
    if env::args().nth(1).as_deref() == Some("healthcheck") {
        return healthcheck();
    }

    let bind = env::var("COLIBRI_BIND").unwrap_or_else(|_| DEFAULT_BIND.into());
    let listener = match TcpListener::bind(&bind) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("colibri: no se pudo escuchar en {bind}: {error}");
            return ExitCode::FAILURE;
        }
    };

    println!("colibri: escuchando en {bind}");
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(|| handle_connection(stream));
            }
            Err(error) => eprintln!("colibri: conexión fallida: {error}"),
        }
    }

    ExitCode::SUCCESS
}

fn handle_connection(mut stream: TcpStream) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut request = [0_u8; 8192];
    let Ok(read) = stream.read(&mut request) else {
        return;
    };
    let request_line = request[..read].split(|byte| *byte == b'\n').next();

    let (status, body) = if request_line
        .is_some_and(|line| line.strip_suffix(b"\r") == Some(b"GET /health HTTP/1.1"))
    {
        ("200 OK", r#"{"status":"ok","engine":"colibri"}"#)
    } else {
        ("404 Not Found", r#"{"error":"not found"}"#)
    };

    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

fn healthcheck() -> ExitCode {
    let addr = env::var("COLIBRI_HEALTH_ADDR").unwrap_or_else(|_| DEFAULT_HEALTH_ADDR.into());
    let Ok(mut stream) = TcpStream::connect_timeout(
        &match addr.parse() {
            Ok(addr) => addr,
            Err(_) => return ExitCode::FAILURE,
        },
        Duration::from_secs(2),
    ) else {
        return ExitCode::FAILURE;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    if stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .is_err()
    {
        return ExitCode::FAILURE;
    }

    let mut response = [0_u8; 64];
    match stream.read(&mut response) {
        Ok(read) if response[..read].starts_with(b"HTTP/1.1 200") => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}
