//! HTTPS listener: TLS 1.3 only, handshakes run in their own tasks with a time limit so one slow
//! client cannot block the others, and the number of open connections is capped.

use rustls::{ServerConfig, version::TLS13};
use rustls_pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
use std::{
    io,
    net::SocketAddr,
    path::Path,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::{TcpListener, TcpStream},
    sync::{OwnedSemaphorePermit, Semaphore, mpsc},
    time::timeout,
};
use tokio_rustls::{TlsAcceptor, server::TlsStream};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_CONNECTIONS: usize = 256;

pub fn load_config(cert: &Path, key: &Path) -> Result<Arc<ServerConfig>, String> {
    let certs = CertificateDer::pem_file_iter(cert)
        .map_err(|e| format!("cannot read certificate {}: {e}", cert.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("bad certificate in {}: {e}", cert.display()))?;
    if certs.is_empty() {
        return Err(format!("no certificate found in {}", cert.display()));
    }
    let key = PrivateKeyDer::from_pem_file(key)
        .map_err(|e| format!("cannot read key {}: {e}", key.display()))?;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&TLS13])
        .map_err(|e| e.to_string())?
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| format!("certificate and key do not match: {e}"))?;
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(Arc::new(config))
}

/// A TLS stream that keeps its connection slot until it is dropped.
pub struct Connection {
    inner: TlsStream<TcpStream>,
    _slot: OwnedSemaphorePermit,
}

impl AsyncRead for Connection {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for Connection {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

pub struct TlsListener {
    ready: mpsc::Receiver<(Connection, SocketAddr)>,
    local: SocketAddr,
}

impl TlsListener {
    pub async fn bind(addr: SocketAddr, config: Arc<ServerConfig>) -> io::Result<TlsListener> {
        let tcp = TcpListener::bind(addr).await?;
        let local = tcp.local_addr()?;
        let (tx, ready) = mpsc::channel(MAX_CONNECTIONS);
        let acceptor = TlsAcceptor::from(config);
        let slots = Arc::new(Semaphore::new(MAX_CONNECTIONS));
        tokio::spawn(async move {
            loop {
                let (stream, peer) = match tcp.accept().await {
                    Ok(x) => x,
                    Err(_) => {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        continue;
                    }
                };
                // Over the connection cap: refuse at once instead of queueing.
                let Ok(slot) = slots.clone().try_acquire_owned() else {
                    continue;
                };
                let (acceptor, tx) = (acceptor.clone(), tx.clone());
                tokio::spawn(async move {
                    if let Ok(Ok(inner)) = timeout(HANDSHAKE_TIMEOUT, acceptor.accept(stream)).await
                    {
                        let _ = tx.send((Connection { inner, _slot: slot }, peer)).await;
                    }
                });
            }
        });
        Ok(TlsListener { ready, local })
    }
}

impl axum::serve::Listener for TlsListener {
    type Io = Connection;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        match self.ready.recv().await {
            Some(x) => x,
            None => std::future::pending().await,
        }
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        Ok(self.local)
    }
}
