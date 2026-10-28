//! A tiny HTTP/1.1 GET over plain TCP, used for the health checks of the local services (loopback only).
//! It keeps the dependency list short: no HTTP client library is needed for a status code.

use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};

const MAX_RESPONSE: usize = 64 * 1024;

/// Returns the HTTP status code and the (possibly truncated) body of `GET path` on `addr`.
pub async fn get(addr: &str, path: &str, limit: Duration) -> Result<(u16, String), String> {
    timeout(limit, async {
        let mut stream = TcpStream::connect(addr).await.map_err(|e| e.to_string())?;
        let request = format!(
            "GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nAccept: */*\r\n\r\n"
        );
        stream
            .write_all(request.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        let mut raw = Vec::new();
        stream
            .take(MAX_RESPONSE as u64)
            .read_to_end(&mut raw)
            .await
            .map_err(|e| e.to_string())?;
        parse(&raw)
    })
    .await
    .map_err(|_| "timeout".to_string())?
}

fn parse(raw: &[u8]) -> Result<(u16, String), String> {
    let text = String::from_utf8_lossy(raw);
    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    let status_line = head.lines().next().ok_or("empty response")?;
    let code = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .ok_or("bad status line")?;
    Ok((code, body.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    async fn fake(response: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 512];
            let _ = s.read(&mut buf).await;
            s.write_all(response.as_bytes()).await.unwrap();
        });
        addr
    }

    #[tokio::test]
    async fn reads_status_and_body() {
        let addr = fake("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok").await;
        assert_eq!(
            get(&addr, "/health", Duration::from_secs(2)).await.unwrap(),
            (200, "ok".into())
        );
    }

    #[tokio::test]
    async fn reports_error_statuses() {
        let addr = fake("HTTP/1.1 503 Service Unavailable\r\n\r\n").await;
        assert_eq!(
            get(&addr, "/", Duration::from_secs(2)).await.unwrap().0,
            503
        );
    }

    #[tokio::test]
    async fn refused_connection_is_an_error() {
        assert!(
            get("127.0.0.1:1", "/", Duration::from_secs(2))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn silent_server_times_out() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        tokio::spawn(async move {
            let (_s, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(5)).await;
        });
        assert_eq!(
            get(&addr, "/", Duration::from_millis(200))
                .await
                .unwrap_err(),
            "timeout"
        );
    }
}
