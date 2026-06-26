use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

/// Max bytes read from one bar TCP connection (LLM reaction text).
pub const MAX_MESSAGE_BYTES: usize = 64 * 1024;

const READ_SLACK: usize = 1024;

/// Read one message from a bar TCP connection, capped at [`MAX_MESSAGE_BYTES`].
pub async fn read_message(stream: &mut TcpStream) -> std::io::Result<Option<String>> {
    let limit = MAX_MESSAGE_BYTES.saturating_add(READ_SLACK) as u64;
    let mut limited = stream.take(limit);
    let mut buf = Vec::new();
    match limited.read_to_end(&mut buf).await {
        Ok(0) => Ok(None),
        Ok(_) => {
            let text = String::from_utf8_lossy(&buf).trim().to_string();
            if text.is_empty() {
                Ok(None)
            } else {
                Ok(Some(text))
            }
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn read_message_returns_trimmed_text() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            read_message(&mut stream).await.unwrap()
        });

        let mut client = TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .unwrap();
        client.write_all(b"  hello chi\n").await.unwrap();
        client.shutdown().await.unwrap();

        assert_eq!(server.await.unwrap(), Some("hello chi".into()));
    }

    #[tokio::test]
    async fn read_message_empty_connection_returns_none() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            read_message(&mut stream).await.unwrap()
        });

        let client = TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .unwrap();
        drop(client);

        assert_eq!(server.await.unwrap(), None);
    }

    #[tokio::test]
    async fn read_message_caps_oversized_payload() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            read_message(&mut stream).await.unwrap()
        });

        let mut client = TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .unwrap();
        let payload = "x".repeat(MAX_MESSAGE_BYTES + 10_000);
        client.write_all(payload.as_bytes()).await.unwrap();
        client.shutdown().await.unwrap();

        let text = server.await.unwrap().unwrap();
        assert!(text.len() <= MAX_MESSAGE_BYTES + READ_SLACK);
    }
}
