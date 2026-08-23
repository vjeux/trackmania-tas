// dsprobe -- probe Nadeo's dedicated-server download host for which dated builds exist.
// Raw HTTP/1.1 HEAD through fwdproxy (absolute-URI form), no crates.
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::thread;

const PROXY: &str = "fwdproxy:8080";
const HOST: &str = "files.v04.maniaplanet.com";

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

struct Hit {
    name: String,
    status: u32,
    len: u64,
    lastmod: String,
}

fn head(path: &str) -> std::io::Result<(u32, u64, String)> {
    let mut s = TcpStream::connect(PROXY)?;
    s.set_read_timeout(Some(std::time::Duration::from_secs(30)))?;
    let req = format!(
        "HEAD http://{h}{p} HTTP/1.1\r\nHost: {h}\r\nConnection: close\r\nUser-Agent: dsprobe\r\n\r\n",
        h = HOST,
        p = path
    );
    s.write_all(req.as_bytes())?;
    let mut r = BufReader::new(s);
    let mut line = String::new();
    r.read_line(&mut line)?;
    let status: u32 = line
        .split_whitespace()
        .nth(1)
        .and_then(|x| x.parse().ok())
        .unwrap_or(0);
    let mut len = 0u64;
    let mut lm = String::new();
    loop {
        let mut l = String::new();
        if r.read_line(&mut l)? == 0 {
            break;
        }
        let t = l.trim_end();
        if t.is_empty() {
            break;
        }
        let low = t.to_ascii_lowercase();
        if let Some(v) = low.strip_prefix("content-length:") {
            len = v.trim().parse().unwrap_or(0);
        }
        if low.starts_with("last-modified:") {
            lm = t[t.find(':').unwrap() + 1..].trim().to_string();
        }
    }
    Ok((status, len, lm))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // usage: dsprobe <mode> ...
    //   dates <y0> <y1> <prefix> <dir>   -- sweep YYYY-MM-DD names
    //   names <dir> <name>...            -- probe explicit names
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("dates");
    let mut names: Vec<String> = Vec::new();
    let dir;
    match mode {
        "dates" => {
            let y0: i32 = args[2].parse().unwrap();
            let y1: i32 = args[3].parse().unwrap();
            let prefix = args[4].clone();
            dir = args[5].clone();
            for y in y0..=y1 {
                for m in 1..=12u32 {
                    for d in 1..=days_in_month(y, m) {
                        names.push(format!("{prefix}{y}-{m:02}-{d:02}.zip"));
                    }
                }
            }
        }
        "names" => {
            dir = args[2].clone();
            for a in &args[3..] {
                names.push(a.clone());
            }
        }
        _ => {
            eprintln!("bad mode");
            std::process::exit(2);
        }
    }

    let nthreads = 16usize;
    let (tx, rx) = mpsc::channel::<Hit>();
    let chunks: Vec<Vec<String>> = (0..nthreads)
        .map(|i| {
            names
                .iter()
                .enumerate()
                .filter(|(j, _)| j % nthreads == i)
                .map(|(_, n)| n.clone())
                .collect()
        })
        .collect();
    let mut hs = Vec::new();
    for c in chunks {
        let tx = tx.clone();
        let dir = dir.clone();
        hs.push(thread::spawn(move || {
            for n in c {
                let p = format!("{dir}{n}");
                let mut attempt = 0;
                loop {
                    match head(&p) {
                        Ok((st, len, lm)) => {
                            if st != 404 {
                                let _ = tx.send(Hit {
                                    name: n.clone(),
                                    status: st,
                                    len,
                                    lastmod: lm,
                                });
                            }
                            break;
                        }
                        Err(e) => {
                            attempt += 1;
                            if attempt >= 3 {
                                eprintln!("ERR {p}: {e}");
                                break;
                            }
                            thread::sleep(std::time::Duration::from_millis(300));
                        }
                    }
                }
            }
        }));
    }
    drop(tx);
    let mut hits: Vec<Hit> = rx.iter().collect();
    for h in hs {
        let _ = h.join();
    }
    hits.sort_by(|a, b| a.name.cmp(&b.name));
    println!("probed {} names under {}", names.len(), dir);
    for h in &hits {
        println!("{:>4}  {:>12}  {}  {}", h.status, h.len, h.lastmod, h.name);
    }
    println!("{} non-404", hits.len());
}
