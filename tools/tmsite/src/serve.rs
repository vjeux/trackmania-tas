//! A ~60-line static file server, so the built page can be fetched over HTTP
//! exactly as a browser would fetch it (the page itself needs no server -- this
//! exists for the verification step).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;

pub fn serve(root: &str, port: u16, requests: usize) -> Result<(), String> {
    let l = TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("bind {}: {}", port, e))?;
    eprintln!("serving {} on http://127.0.0.1:{}/", root, port);
    let mut served = 0usize;
    for c in l.incoming() {
        let mut c = match c {
            Ok(c) => c,
            Err(_) => continue,
        };
        let mut br = BufReader::new(c.try_clone().map_err(|e| e.to_string())?);
        let mut line = String::new();
        if br.read_line(&mut line).is_err() {
            continue;
        }
        // drain headers
        loop {
            let mut h = String::new();
            if br.read_line(&mut h).unwrap_or(0) <= 2 {
                break;
            }
        }
        let path = line.split_whitespace().nth(1).unwrap_or("/").to_string();
        let rel = path.trim_start_matches('/').split('?').next().unwrap_or("").to_string();
        let file = std::path::Path::new(root).join(if rel.is_empty() { "index.html" } else { &rel });
        let body = std::fs::File::open(&file).and_then(|mut f| {
            let mut v = Vec::new();
            f.read_to_end(&mut v)?;
            Ok(v)
        });
        let resp = match body {
            Ok(b) => {
                let ct = if file.extension().map(|e| e == "html").unwrap_or(false) {
                    "text/html; charset=utf-8"
                } else {
                    "application/octet-stream"
                };
                let mut h = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    ct,
                    b.len()
                )
                .into_bytes();
                h.extend_from_slice(&b);
                h
            }
            Err(_) => b"HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\nConnection: close\r\n\r\nnot found".to_vec(),
        };
        let _ = c.write_all(&resp);
        let _ = c.flush();
        served += 1;
        if requests > 0 && served >= requests {
            break;
        }
    }
    Ok(())
}
