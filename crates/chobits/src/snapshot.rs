use chobits_meta::http::{content_length_from_headers, header_body_split, is_http_post};
use chobits_meta::snapshot::truncate_snapshot;
use tokio::io::AsyncReadExt;

/// Extra bytes read beyond `max_bytes` before truncation (one JSON field, etc.).
const READ_SLACK: usize = 1024;

/// Accept `POST /snapshot` HTTP requests on the given localhost port.
pub async fn listen(
    port: u16,
    max_bytes: usize,
    mut handler: impl FnMut(String),
) -> Result<(), Box<dyn std::error::Error>> {
    let addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!(
        "[snapshot] HTTP listening on {} (max {} bytes)",
        addr, max_bytes
    );

    let read_cap = max_bytes.saturating_add(READ_SLACK);
    let read_limit = read_cap as u64;

    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                eprintln!("[snapshot] Accept error: {}", e);
                continue;
            }
        };

        match read_http_payload(stream, read_limit).await {
            Ok(Some(raw)) => deliver_payload(&raw, max_bytes, &mut handler),
            Ok(None) => {}
            Err(e) => eprintln!("[snapshot] Read error from {}: {}", peer, e),
        }
    }
}

fn deliver_payload(raw: &str, max_bytes: usize, handler: &mut impl FnMut(String)) {
    if raw.is_empty() {
        return;
    }

    let original_len = raw.len();
    let text = truncate_snapshot(raw, max_bytes);

    if original_len > max_bytes {
        println!(
            "[snapshot] Truncated {} → {} bytes",
            original_len,
            text.len()
        );
    }

    handler(text);
}

async fn read_http_payload(
    mut stream: impl tokio::io::AsyncRead + Unpin,
    read_limit: u64,
) -> std::io::Result<Option<String>> {
    let mut peek = vec![0u8; 512];
    let n = stream.read(&mut peek).await?;
    if n == 0 {
        return Ok(None);
    }
    peek.truncate(n);

    if !is_http_post(&peek) {
        return Ok(None);
    }

    let mut body = read_http_body(&mut stream, &peek, read_limit).await?;
    if body.len() as u64 > read_limit {
        body.truncate(read_limit as usize);
    }

    let text = String::from_utf8_lossy(&body).trim().to_string();
    Ok(if text.is_empty() { None } else { Some(text) })
}

async fn read_http_body(
    stream: &mut (impl tokio::io::AsyncRead + Unpin),
    initial: &[u8],
    read_limit: u64,
) -> std::io::Result<Vec<u8>> {
    let mut buf = initial.to_vec();
    while header_body_split(&buf).is_none() {
        if buf.len() as u64 >= read_limit {
            break;
        }
        let mut chunk = vec![0u8; 1024];
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        chunk.truncate(n);
        buf.extend_from_slice(&chunk);
    }

    let Some(body_start) = header_body_split(&buf) else {
        return Ok(Vec::new());
    };

    let headers_end = body_start.saturating_sub(4);
    let content_len = content_length_from_headers(&buf[..headers_end])
        .unwrap_or(0)
        .min(read_limit as usize);

    let mut body = buf[body_start..].to_vec();
    while body.len() < content_len && (body.len() as u64) < read_limit {
        let mut chunk = vec![0u8; content_len.saturating_sub(body.len()).min(4096)];
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        chunk.truncate(n);
        body.extend_from_slice(&chunk);
    }
    body.truncate(content_len);
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn accepts_http_post_body() {
        let (mut client, server) = tokio::io::duplex(4096);
        let req = b"POST /snapshot HTTP/1.1\r\nContent-Length: 11\r\n\r\nhello world";
        client.write_all(req).await.unwrap();
        client.shutdown().await.unwrap();
        let payload = read_http_payload(server, 4096).await.unwrap();
        assert_eq!(payload.as_deref(), Some("hello world"));
    }

    #[tokio::test]
    async fn ignores_non_http_traffic() {
        let (mut client, server) = tokio::io::duplex(4096);
        client.write_all(b"{\"screen\":\"hi\"}").await.unwrap();
        client.shutdown().await.unwrap();
        let payload = read_http_payload(server, 4096).await.unwrap();
        assert_eq!(payload, None);
    }

    #[test]
    fn truncate_output_within_max_bytes() {
        use chobits_meta::snapshot::truncate_snapshot as truncate;
        let s = "x\n".repeat(5000);
        let max = 4096;
        let out = truncate(&s, max);
        assert!(out.len() <= max);
    }
}
