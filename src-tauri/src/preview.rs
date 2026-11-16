//! Stream selected media over loopback HTTP. WKWebView does not reliably
//! decode large MP4s through a custom scheme with capped byte-range replies.
//! Each response reads at most 64 KiB at a time, including non-range GETs.
use http_range::HttpRange;
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

#[derive(Clone)]
struct Selection {
    path: PathBuf,
    route: String,
    mime: &'static str,
}

#[derive(Default)]
struct Shared {
    selected: Mutex<Option<Selection>>,
    stopped: AtomicBool,
    active: AtomicUsize,
}

pub struct PreviewServer {
    address: SocketAddr,
    shared: Arc<Shared>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl PreviewServer {
    pub fn start() -> io::Result<Self> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
        let address = listener.local_addr()?;
        let shared = Arc::new(Shared::default());
        let state = shared.clone();
        let worker = thread::Builder::new()
            .name("media-preview".into())
            .spawn(move || {
                let mut connections: Vec<JoinHandle<()>> = Vec::new();
                for stream in listener.incoming() {
                    if state.stopped.load(Ordering::Acquire) {
                        break;
                    }
                    let Ok(mut stream) = stream else {
                        continue;
                    };
                    if state.active.fetch_add(1, Ordering::AcqRel) >= 8 {
                        state.active.fetch_sub(1, Ordering::Release);
                        let _ = stream.set_write_timeout(Some(Duration::from_millis(100)));
                        let _ = empty_response(&mut stream, 503, "Service Unavailable", "");
                        continue;
                    }
                    connections.retain(|thread| !thread.is_finished());
                    let state = state.clone();
                    connections.push(thread::spawn(move || {
                        let _ = serve(&mut stream, address, &state);
                        state.active.fetch_sub(1, Ordering::Release);
                    }));
                }
                for connection in connections {
                    let _ = connection.join();
                }
            })?;
        Ok(Self {
            address,
            shared,
            worker: Mutex::new(Some(worker)),
        })
    }

    /// URLs contain only a fresh random token, never a filesystem path.
    /// Replacing the selection revokes the previous URL and stops its streams.
    pub fn select(&self, path: &Path) -> io::Result<String> {
        let path = path.canonicalize()?;
        if !path.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Choose a video file.",
            ));
        }
        let mut bytes = [0_u8; 24];
        getrandom::fill(&mut bytes).map_err(|e| io::Error::other(e.to_string()))?;
        let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let route = format!("/media/{token}");
        let mime = match path
            .extension()
            .and_then(|p| p.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "mp4" | "m4v" => "video/mp4",
            "mov" => "video/quicktime",
            "webm" => "video/webm",
            "mkv" => "video/x-matroska",
            "avi" => "video/x-msvideo",
            "ts" | "mts" | "m2ts" => "video/mp2t",
            _ => "application/octet-stream",
        };
        *self
            .shared
            .selected
            .lock()
            .map_err(|_| io::Error::other("preview state poisoned"))? = Some(Selection {
            path,
            route: route.clone(),
            mime,
        });
        Ok(format!("http://{}{route}", self.address))
    }

    pub fn shutdown(&self) {
        if !self.shared.stopped.swap(true, Ordering::AcqRel) {
            // Wake the blocking accept without polling while the app is idle.
            let _ = TcpStream::connect_timeout(&self.address, Duration::from_millis(100));
        }
        if let Ok(mut worker) = self.worker.lock() {
            if let Some(worker) = worker.take() {
                let _ = worker.join();
            }
        }
    }
}

impl Drop for PreviewServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn empty_response(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    extra: &str,
) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Length: 0\r\nConnection: close\r\n{extra}\r\n"
    )
}

fn serve(stream: &mut TcpStream, address: SocketAddr, state: &Shared) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    let mut request = Vec::with_capacity(1024);
    let mut buffer = [0_u8; 1024];
    loop {
        let n = stream.read(&mut buffer)?;
        if n == 0 {
            return Ok(());
        }
        request.extend_from_slice(&buffer[..n]);
        if request.windows(4).any(|b| b == b"\r\n\r\n") {
            break;
        }
        if request.len() >= 8192 {
            return empty_response(stream, 431, "Request Header Fields Too Large", "");
        }
    }
    let Ok(request) = std::str::from_utf8(&request) else {
        return empty_response(stream, 400, "Bad Request", "");
    };
    let mut lines = request.split("\r\n");
    let mut first = lines.next().unwrap_or("").split_whitespace();
    let (method, route) = (first.next().unwrap_or(""), first.next().unwrap_or(""));
    if !["GET", "HEAD"].contains(&method) {
        return empty_response(stream, 405, "Method Not Allowed", "Allow: GET, HEAD\r\n");
    }
    let mut range = None;
    let mut host = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.eq_ignore_ascii_case("range") {
            range = Some(value.trim());
        }
        if name.eq_ignore_ascii_case("host") {
            host = Some(value.trim());
        }
    }
    if host != Some(address.to_string().as_str()) {
        return empty_response(stream, 403, "Forbidden", "");
    }
    let selected = state
        .selected
        .lock()
        .map_err(|_| io::Error::other("preview state poisoned"))?
        .clone();
    let Some(selected) = selected.filter(|s| s.route == route) else {
        return empty_response(stream, 404, "Not Found", "");
    };
    let mut file = match File::open(&selected.path) {
        Ok(file) => file,
        Err(_) => return empty_response(stream, 404, "Not Found", ""),
    };
    let size = file.metadata()?.len();
    let part = if method == "GET" {
        if let Some(range) = range {
            match HttpRange::parse(range, size) {
                Ok(ranges) if ranges.len() == 1 => Some(ranges[0]),
                _ => {
                    return empty_response(
                        stream,
                        416,
                        "Range Not Satisfiable",
                        &format!("Content-Range: bytes */{size}\r\n"),
                    )
                }
            }
        } else {
            None
        }
    } else {
        None
    };
    let (start, length) = part.map(|p| (p.start, p.length)).unwrap_or((0, size));
    if part.is_some() {
        write!(
            stream,
            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{}/{size}\r\n",
            start + length - 1
        )?;
    } else {
        write!(stream, "HTTP/1.1 200 OK\r\n")?;
    }
    write!(stream, "Content-Type: {}\r\nContent-Length: {length}\r\nAccept-Ranges: bytes\r\nAccess-Control-Allow-Origin: *\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n", selected.mime)?;
    if method == "HEAD" {
        return Ok(());
    }
    file.seek(SeekFrom::Start(start))?;
    let mut remaining = length;
    let mut bytes = [0_u8; 64 * 1024];
    while remaining > 0 && !state.stopped.load(Ordering::Acquire) {
        if !state
            .selected
            .lock()
            .map_err(|_| io::Error::other("preview state poisoned"))?
            .as_ref()
            .is_some_and(|s| s.route == selected.route)
        {
            break;
        }
        let capacity = (remaining as usize).min(bytes.len());
        let count = file.read(&mut bytes[..capacity])?;
        if count == 0 {
            break;
        }
        stream.write_all(&bytes[..count])?;
        remaining -= count as u64;
    }
    Ok(())
}
