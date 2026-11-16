use minimal_video_editor::preview::PreviewServer;
use std::{
    io::{Read, Seek, SeekFrom, Write},
    net::TcpStream,
};

fn request(url: &str, method: &str, range: Option<&str>) -> (String, Vec<u8>) {
    let (host, path) = url
        .strip_prefix("http://")
        .unwrap()
        .split_once('/')
        .unwrap();
    let mut stream = TcpStream::connect(host).unwrap();
    write!(
        stream,
        "{method} /{path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n"
    )
    .unwrap();
    if let Some(range) = range {
        write!(stream, "Range: {range}\r\n").unwrap();
    }
    stream.write_all(b"\r\n").unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    let split = response
        .windows(4)
        .position(|bytes| bytes == b"\r\n\r\n")
        .unwrap();
    (
        String::from_utf8(response[..split].to_vec()).unwrap(),
        response[split + 4..].to_vec(),
    )
}

#[test]
fn serves_exact_large_ranges_without_the_asset_protocol_one_megabyte_limit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("日本語 sample's video.mp4");
    std::fs::write(&path, vec![42_u8; 3 * 1024 * 1024]).unwrap();
    let server = PreviewServer::start().unwrap();
    let url = server.select(&path).unwrap();
    let (headers, body) = request(&url, "GET", Some("bytes=0-2097151"));
    assert!(headers.starts_with("HTTP/1.1 206"));
    assert!(headers.contains("Content-Type: video/mp4"));
    assert!(headers.contains("Content-Range: bytes 0-2097151/3145728"));
    assert_eq!(body.len(), 2097152);
    assert!(body.iter().all(|b| *b == 42));
}

#[test]
fn probes_headers_and_streams_the_tail_of_a_large_file_with_suffix_ranges() {
    let mut file = tempfile::Builder::new().suffix(".mp4").tempfile().unwrap();
    let size = 875_394_906_u64;
    file.as_file_mut().set_len(size).unwrap();
    file.seek(SeekFrom::End(-4)).unwrap();
    file.write_all(b"tail").unwrap();
    let server = PreviewServer::start().unwrap();
    let url = server.select(file.path()).unwrap();
    let (headers, body) = request(&url, "HEAD", None);
    assert!(headers.starts_with("HTTP/1.1 200"));
    assert!(headers.contains("Content-Length: 875394906"));
    assert!(body.is_empty());
    let (headers, body) = request(&url, "GET", Some("bytes=-4"));
    assert!(headers.starts_with("HTTP/1.1 206"));
    assert_eq!(body, b"tail");
    assert_eq!(file.as_file().metadata().unwrap().len(), size);
}

#[test]
fn rejects_invalid_ranges_and_stale_or_forged_media_tokens() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.mp4");
    std::fs::write(&path, b"12345678").unwrap();
    let server = PreviewServer::start().unwrap();
    let old = server.select(&path).unwrap();
    assert!(request(&old, "GET", Some("bytes=100-200"))
        .0
        .starts_with("HTTP/1.1 416"));
    let current = server.select(&path).unwrap();
    assert_ne!(old, current);
    assert!(request(&old, "GET", None).0.starts_with("HTTP/1.1 404"));
    let forged = format!("{}/../../etc/passwd", current.rsplit_once('/').unwrap().0);
    assert!(request(&forged, "GET", None).0.starts_with("HTTP/1.1 404"));
    assert_eq!(request(&current, "GET", None).1, b"12345678");
}
