//! Pero server. The only process that listens on the network (HTTPS, port 8443).
//! llama.cpp and LanguageTool stay on 127.0.0.1; this server only asks them whether they are alive.

mod config;
mod db;
mod health;
mod http_probe;
mod tls;

use std::process::ExitCode;

const DEFAULT_CONFIG: &str = "/etc/pero/server.toml";

fn config_path() -> Result<String, String> {
    let mut args = std::env::args().skip(1);
    match (args.next().as_deref(), args.next()) {
        (None, _) => Ok(DEFAULT_CONFIG.to_string()),
        (Some("--config"), Some(path)) => Ok(path),
        (Some("--version"), _) => {
            println!("pero-server {}", env!("CARGO_PKG_VERSION"));
            Err(String::new())
        }
        _ => Err("usage: pero-server [--config PATH] | --version".to_string()),
    }
}

async fn shutdown() {
    use tokio::signal::unix::{SignalKind, signal};
    let mut term = signal(SignalKind::terminate()).expect("install SIGTERM handler");
    tokio::select! {
        _ = term.recv() => {}
        _ = tokio::signal::ctrl_c() => {}
    }
}

async fn run() -> Result<(), String> {
    let path = config_path()?;
    let cfg = config::Config::load(&path)?;
    let tls = tls::load_config(&cfg.tls_cert, &cfg.tls_key)?;
    let listener = tls::TlsListener::bind(cfg.bind, tls)
        .await
        .map_err(|e| format!("cannot listen on {}: {e}", cfg.bind))?;
    eprintln!(
        "pero-server {} listening on https://{}",
        env!("CARGO_PKG_VERSION"),
        cfg.bind
    );
    let app = health::router(health::AppState::new(cfg.llm, cfg.languagetool));
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown())
        .await
        .map_err(|e| e.to_string())?;
    eprintln!("pero-server stopped");
    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) if e.is_empty() => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("pero-server: {e}");
            ExitCode::FAILURE
        }
    }
}
