//! Pero desktop app: the web client in a window. The page asks this program to talk to the server (see server.rs);
//! the device token is kept in the system keychain and never reaches the page.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod rules;
mod secrets;
mod server;
mod settings;

use server::{Found, Response, Session, State};
use tauri::Manager;

#[tauri::command]
async fn discover(state: tauri::State<'_, State>) -> Result<Found, String> {
    state.discover().await
}

#[tauri::command]
async fn use_address(state: tauri::State<'_, State>, address: String) -> Result<Found, String> {
    state.use_address(&address).await
}

#[tauri::command]
async fn api_request(
    state: tauri::State<'_, State>,
    method: String,
    path: String,
    body: Option<String>,
) -> Result<Response, String> {
    state.request(&method, &path, body).await
}

#[tauri::command]
async fn sign_in(state: tauri::State<'_, State>, token: String) -> Result<Session, String> {
    state.sign_in(&token).await
}

#[tauri::command]
async fn resume(state: tauri::State<'_, State>) -> Result<Option<Session>, String> {
    state.resume().await
}

#[tauri::command]
fn sign_out(state: tauri::State<'_, State>) {
    state.sign_out();
}

/// `pero-desktop --check`: no window; looks for the server over the pinned CA and tries the keychain, then says what it found.
/// Prints nothing secret. Exit code 0 only when both work.
fn check(config_dir: std::path::PathBuf) -> std::process::ExitCode {
    let state = match State::new(config_dir) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("FAIL  {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let mut ok = true;
    match tauri::async_runtime::block_on(state.discover()) {
        Ok(found) => println!(
            "PASS  found the server at {} (version {}) over the pinned CA",
            found.address, found.version
        ),
        Err(e) => {
            ok = false;
            println!("FAIL  {e}");
        }
    }
    match secrets::probe() {
        Ok(()) => println!("PASS  the system keychain stores and returns a secret"),
        Err(e) => {
            ok = false;
            println!("FAIL  {e}");
        }
    }
    if ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::FAILURE
    }
}

fn main() -> std::process::ExitCode {
    // One crypto provider for the whole process (the same one the server uses).
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("no other crypto provider is installed yet");
    if std::env::args().any(|a| a == "--check") {
        let home = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_default();
        return check(home.join(".config/local.pero.desktop"));
    }
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            app.manage(State::new(dir)?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            discover,
            use_address,
            api_request,
            sign_in,
            resume,
            sign_out
        ])
        .run(tauri::generate_context!())
        .expect("the desktop app could not start");
    std::process::ExitCode::SUCCESS
}
