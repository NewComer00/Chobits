use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

/// Send a text line to the bar TCP endpoint.
pub async fn send_text(port: u16, text: &str) -> Result<(), Box<dyn std::error::Error>> {
    let addr = format!("127.0.0.1:{}", port);
    match TcpStream::connect(&addr).await {
        Ok(mut stream) => {
            stream.write_all(text.as_bytes()).await?;
            stream.write_all(b"\n").await?;
            stream.shutdown().await?;
            Ok(())
        }
        Err(e) => {
            eprintln!("[bar] Failed to connect to {}: {}", addr, e);
            Err(Box::new(e))
        }
    }
}
