//! A small read-only HTTP listener over the parcel store.
//!
//! It answers three things: `GET /parcels/<id>`, `GET /health`, and
//! everything else with an error. It reads the store file afresh on every
//! request, so changes made from the command line show up without a restart.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};

use crate::config::Config;
use crate::store::Store;

/// The longest request line the listener will read, in bytes.
const MAX_REQUEST_LINE: u64 = 8192;

/// The status code and body for one request.
pub fn route(store: &Store, method: &str, path: &str) -> (u16, String) {
    if method != "GET" {
        return (405, "method not allowed\n".to_owned());
    }
    if path == "/health" {
        return (200, "ok\n".to_owned());
    }
    if let Some(id) = path.strip_prefix("/parcels/") {
        return match store.get(id) {
            Some(parcel) => (200, format!("{}\t{}\n", parcel.id, parcel.status)),
            None => (404, "not found\n".to_owned()),
        };
    }
    (404, "not found\n".to_owned())
}

/// Bind the loopback interface on the configured port and serve until the
/// process is stopped.
pub fn serve(config: &Config) -> io::Result<()> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, config.port))?;
    println!("listening on 127.0.0.1:{}", config.port);
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle(stream, config) {
                    eprintln!("request failed: {error}");
                }
            }
            Err(error) => eprintln!("accept failed: {error}"),
        }
    }
    Ok(())
}

fn handle(mut stream: TcpStream, config: &Config) -> io::Result<()> {
    let mut reader = BufReader::new((&stream).take(MAX_REQUEST_LINE));
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut parts = request_line.split_whitespace();
    let (method, path) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));

    let (code, body) = match Store::open(&config.data_file) {
        Ok(store) => route(&store, method, path),
        Err(error) => (500, format!("{error}\n")),
    };
    let reason = match code {
        200 => "OK",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Internal Server Error",
    };
    write!(
        stream,
        "HTTP/1.1 {code} {reason}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}
