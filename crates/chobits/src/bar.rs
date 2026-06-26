use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

/// Send a text line to the bar TCP endpoint.
pub async fn send_text(port: u16, text: &str) -> Result<(), Box<dyn std::error::Error>> {
    let addr = format!("127.0.0.1:{}", port);
    let mut stream = TcpStream::connect(&addr).await.map_err(|e| {
        eprintln!("[bar] Failed to connect to {}: {}", addr, e);
        e
    })?;
    stream.write_all(text.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    stream.shutdown().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn send_text_delivers_newline_terminated_payload() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let recv = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = Vec::new();
            stream.read_to_end(&mut buf).await.unwrap();
            buf
        });

        send_text(port, "hello chi").await.unwrap();
        let buf = recv.await.unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "hello chi\n");
    }

    #[tokio::test]
    async fn send_text_supports_utf8() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let recv = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = Vec::new();
            stream.read_to_end(&mut buf).await.unwrap();
            buf
        });

        send_text(port, "こんにちは ♪").await.unwrap();
        let buf = recv.await.unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), "こんにちは ♪\n");
    }

    #[tokio::test]
    async fn send_text_errors_when_connection_refused() {
        // Fixed port with no listener — avoids races from binding/dropping port 0.
        let result = send_text(65000, "x").await;
        assert!(result.is_err());
    }
}
