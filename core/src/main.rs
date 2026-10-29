//! Pero server. The only process that listens on the network (HTTPS, port 8443).
//! llama.cpp and LanguageTool stay on 127.0.0.1; this server only asks them whether they are alive.

mod api;
mod canon;
mod canon_api;
mod check;
mod config;
mod db;
mod device;
mod git;
mod graph;
mod graph_api;
mod health;
mod http_probe;
mod ids;
mod import;
mod manuscript;
mod store;
#[cfg(test)]
mod testkit;
mod tls;
mod web;

use std::process::ExitCode;

const DEFAULT_CONFIG: &str = "/etc/pero/server.toml";

const USAGE: &str = "usage: pero-server [--config PATH] | --version\n       pero-server check-import DIR [--mode blank-line|line] [--language ru|en]\n       pero-server device add NAME | list | revoke NAME-OR-ID  [--config PATH]";

enum DeviceAction {
    Add(String),
    List,
    Revoke(String),
}

enum Command {
    Serve {
        config: String,
    },
    Device {
        config: String,
        action: DeviceAction,
    },
    CheckImport {
        dir: String,
        mode: import::ParagraphMode,
        language: String,
    },
}

fn parse_args() -> Result<Command, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None => Ok(Command::Serve {
            config: DEFAULT_CONFIG.to_string(),
        }),
        Some("--version") => {
            println!("pero-server {}", env!("CARGO_PKG_VERSION"));
            Err(String::new())
        }
        Some("--config") if args.len() == 2 => Ok(Command::Serve {
            config: args[1].clone(),
        }),
        Some("device") => {
            let mut config = DEFAULT_CONFIG.to_string();
            let mut words = Vec::new();
            let mut rest = args[1..].iter();
            while let Some(a) = rest.next() {
                if a == "--config" {
                    config = rest.next().ok_or(USAGE)?.clone();
                } else {
                    words.push(a.as_str());
                }
            }
            let action = match words.as_slice() {
                ["add", name] => DeviceAction::Add(name.to_string()),
                ["list"] => DeviceAction::List,
                ["revoke", who] => DeviceAction::Revoke(who.to_string()),
                _ => return Err(USAGE.to_string()),
            };
            Ok(Command::Device { config, action })
        }
        Some("check-import") => {
            let dir = args
                .get(1)
                .filter(|a| !a.starts_with("--"))
                .ok_or(USAGE)?
                .clone();
            let (mut mode, mut language) = (import::ParagraphMode::BlankLine, "ru".to_string());
            let mut rest = args[2..].iter();
            while let Some(flag) = rest.next() {
                match (flag.as_str(), rest.next().map(String::as_str)) {
                    ("--mode", Some("blank-line")) => mode = import::ParagraphMode::BlankLine,
                    ("--mode", Some("line")) => mode = import::ParagraphMode::Line,
                    ("--language", Some(l @ ("ru" | "en"))) => language = l.to_string(),
                    _ => return Err(USAGE.to_string()),
                }
            }
            Ok(Command::CheckImport {
                dir,
                mode,
                language,
            })
        }
        _ => Err(USAGE.to_string()),
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

async fn check_import(
    dir: &str,
    mode: import::ParagraphMode,
    language: &str,
) -> Result<(), String> {
    let files = check::read_folder(std::path::Path::new(dir))?;
    match check::run_check(files, mode, language).await {
        Ok(report) => {
            print!("{}", report.render());
            if report.passed() {
                Ok(())
            } else {
                Err("the round trip changed some text".to_string())
            }
        }
        Err(check::CheckError::Rejected(lines)) => {
            for l in &lines {
                eprintln!("{l}");
            }
            Err(format!("{} problem(s): nothing was imported", lines.len()))
        }
        Err(check::CheckError::Failed(e)) => Err(e),
    }
}

/// True when this process runs as root (Linux). The database files must belong to the service
/// user, so device commands refuse to create them as root.
fn running_as_root() -> bool {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines().find_map(|l| {
                l.strip_prefix("Uid:")
                    .map(|r| r.split_whitespace().nth(1) == Some("0"))
            })
        })
        .unwrap_or(false)
}

async fn run_device(config: &str, action: DeviceAction) -> Result<(), String> {
    if running_as_root() {
        return Err(
            "run this as the service user (for example: sudo -u pero pero-server device ...), \
             not as root: the database files must stay owned by the service user"
                .to_string(),
        );
    }
    let cfg = config::Config::load(config)?;
    let data_dir = cfg
        .data_dir
        .ok_or("data_dir is not set in the configuration file")?;
    let pool = db::open(&data_dir.join("pero.db")).await?;
    let show = |e: store::StoreError| e.to_string();
    match action {
        DeviceAction::Add(name) => {
            let (info, token) = device::add(&pool, &name).await.map_err(show)?;
            println!("device \"{}\" registered ({})", info.name, info.id);
            println!(
                "token (shown once, it cannot be recovered; put it in the client's password manager):"
            );
            println!("{token}");
        }
        DeviceAction::List => {
            let devices = device::list(&pool).await.map_err(show)?;
            if devices.is_empty() {
                println!("no devices registered");
            }
            for d in devices {
                let state = match &d.revoked_at {
                    Some(at) => format!("revoked {at}"),
                    None => "active".to_string(),
                };
                println!(
                    "{}  {:<24} {}  created {}  last seen {}",
                    d.id,
                    d.name,
                    state,
                    d.created_at,
                    d.last_seen_at.as_deref().unwrap_or("never")
                );
            }
        }
        DeviceAction::Revoke(who) => {
            let info = device::revoke(&pool, &who).await.map_err(show)?;
            println!(
                "device \"{}\" ({}) revoked; its token no longer works",
                info.name, info.id
            );
        }
    }
    pool.close().await;
    Ok(())
}

async fn run() -> Result<(), String> {
    let path = match parse_args()? {
        Command::Serve { config } => config,
        Command::Device { config, action } => return run_device(&config, action).await,
        Command::CheckImport {
            dir,
            mode,
            language,
        } => return check_import(&dir, mode, &language).await,
    };
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
    let mut api_store = None;
    if let Some(data_dir) = &cfg.data_dir {
        // Every API route needs the token of a registered device (see `api::router`).
        let pool = db::open(&data_dir.join("pero.db")).await?;
        let store = store::Store::new(pool, data_dir.join("books"))?;
        if device::count_active(store.pool())
            .await
            .map_err(|e| e.to_string())?
            == 0
        {
            eprintln!(
                "no device registered yet: every API request will be refused until you run \
                 `pero-server device add NAME`"
            );
        }
        api_store = Some(store);
        eprintln!("manuscript API enabled, data in {}", data_dir.display());
    }
    if let Some(web_dir) = &cfg.web_dir {
        web::check(web_dir)?;
        eprintln!("web client served from {}", web_dir.display());
    }
    let app = compose(
        health::router(health::AppState::new(cfg.llm, cfg.languagetool)),
        cfg.web_dir,
        api_store,
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown())
        .await
        .map_err(|e| e.to_string())?;
    eprintln!("pero-server stopped");
    Ok(())
}

/// Puts the parts of the server together. The API goes in last, on purpose: its guard also answers
/// for unknown paths under `/api/v1` (401 without a token), and axum keeps only the fallback of the
/// router that is merged in last.
fn compose(
    base: axum::Router,
    web_dir: Option<std::path::PathBuf>,
    store: Option<std::sync::Arc<store::Store>>,
) -> axum::Router {
    let mut app = base;
    if let Some(dir) = web_dir {
        app = app.merge(web::router(dir));
    }
    if let Some(store) = store {
        app = app.merge(api::router(store));
    }
    app
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
