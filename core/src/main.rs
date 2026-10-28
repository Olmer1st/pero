//! Pero server. The only process that listens on the network (HTTPS, port 8443).
//! llama.cpp and LanguageTool stay on 127.0.0.1; this server only asks them whether they are alive.

mod api;
mod check;
mod config;
mod db;
mod git;
mod health;
mod http_probe;
mod ids;
mod import;
mod manuscript;
mod store;
mod tls;

use std::process::ExitCode;

const DEFAULT_CONFIG: &str = "/etc/pero/server.toml";

const USAGE: &str = "usage: pero-server [--config PATH] | --version\n       pero-server check-import DIR [--mode blank-line|line] [--language ru|en]";

enum Command {
    Serve {
        config: String,
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

async fn run() -> Result<(), String> {
    let path = match parse_args()? {
        Command::Serve { config } => config,
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
    let mut app = health::router(health::AppState::new(cfg.llm, cfg.languagetool));
    if let Some(data_dir) = &cfg.data_dir {
        // The manuscript API has no login until device authentication exists, so it is only mounted
        // when `data_dir` is set: do not set it on a server before that part is installed.
        let pool = db::open(&data_dir.join("pero.db")).await?;
        let store = store::Store::new(pool, data_dir.join("books"))?;
        app = app.merge(api::router(store));
        eprintln!("manuscript API enabled, data in {}", data_dir.display());
    }
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
