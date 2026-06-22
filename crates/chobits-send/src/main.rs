use std::io::{self, Read};
use std::net::TcpStream;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut port: u16 = chobits::Config::load()
        .map(|c| c.ports.snapshot)
        .unwrap_or(7878);
    let mut text: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--port" => {
                if i + 1 < args.len() {
                    port = args[i + 1].parse().unwrap_or(7878);
                    i += 1;
                }
            }
            "--text" => {
                if i + 1 < args.len() {
                    text = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    // If no --text provided, read from stdin
    let payload = match text {
        Some(t) => t,
        None => {
            let mut buf = String::new();
            io::stdin()
                .read_to_string(&mut buf)
                .expect("Failed to read stdin");
            buf.trim().to_string()
        }
    };

    if payload.is_empty() {
        eprintln!("[chobits-send] No input provided");
        std::process::exit(1);
    }

    let addr = format!("127.0.0.1:{}", port);
    match TcpStream::connect(&addr) {
        Ok(mut stream) => {
            use std::io::Write;
            if let Err(e) = stream.write_all(payload.as_bytes()) {
                eprintln!("[chobits-send] Write error: {}", e);
                std::process::exit(1);
            }
            // Shutdown write to signal EOF to the daemon
            let _ = stream.shutdown(std::net::Shutdown::Write);
        }
        Err(e) => {
            eprintln!("[chobits-send] Failed to connect to {}: {}", addr, e);
            std::process::exit(1);
        }
    }
}
