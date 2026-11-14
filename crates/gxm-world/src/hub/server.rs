//! A tiny HTTP/1.1 server on 127.0.0.1: one request per connection, JSON in and out. Enough to
//! stand in for a provider's API at the network edge, and nothing more.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

/// A request, as the handler sees it.
pub struct Request {
    pub method: String,
    pub path: String,
    pub authorization: Option<String>,
    pub body: String,
}

/// Serves until dropped.
pub struct Server {
    pub port: u16,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Server {
    /// Listens on a free port and answers each request with `handle`'s status and JSON.
    pub fn start(handle: impl Fn(Request) -> (u16, String) + Send + 'static) -> std::io::Result<Server> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        let thread = std::thread::Builder::new().name("gxm-world-hub".to_owned()).spawn(move || {
            for stream in listener.incoming() {
                if stopping.load(Ordering::SeqCst) {
                    break;
                }
                if let Ok(stream) = stream {
                    // A client that hangs up mid-request only loses its own answer.
                    serve(stream, &handle).ok();
                }
            }
        })?;
        Ok(Server { port, stop, thread: Some(thread) })
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wake the accept loop so it sees the flag.
        TcpStream::connect(("127.0.0.1", self.port)).ok();
        if let Some(t) = self.thread.take() {
            t.join().ok();
        }
    }
}

fn serve(stream: TcpStream, handle: &impl Fn(Request) -> (u16, String)) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let (method, path) = (parts.next().unwrap_or_default().to_owned(), parts.next().unwrap_or_default().to_owned());
    if method.is_empty() {
        return Ok(());
    }
    let (mut length, mut authorization) = (0usize, None);
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            match name.trim().to_ascii_lowercase().as_str() {
                "content-length" => length = value.trim().parse().unwrap_or(0),
                "authorization" => authorization = Some(value.trim().to_owned()),
                _ => {}
            }
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    let request = Request { method, path, authorization, body: String::from_utf8_lossy(&body).into_owned() };
    let (status, json) = handle(request);
    let reason = match status {
        200 => "OK",
        401 => "Unauthorized",
        404 => "Not Found",
        _ => "Error",
    };
    let mut stream = stream;
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
        json.len()
    )?;
    stream.flush()
}
