# Pero desktop app

The web client (`client/`) in a window, built with Tauri v2 for Linux. It is the same interface as the page the server serves, with three differences: requests are made by the app (not the web view), the app trusts exactly one certificate authority (your home CA, built into the binary), and the device token is kept in the system keychain.

## Build (on the machine that will run it)
System packages (Tauri's official list for Debian/Ubuntu; the others are usually there already):
```bash
sudo apt install libwebkit2gtk-4.1-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev
```
Then, with Node 22 on `PATH` and the home CA's public certificate at `~/.pero-ca/ca.crt` (or `PERO_CA_FILE=...`):
```bash
desktop/build.sh          # checks, tests, audits, builds; the app is desktop/src-tauri/target/release/pero-desktop
desktop/install-launcher.sh   # optional: adds Pero to your application menu (no root)
```
`pero-desktop --check` runs without a window: it looks for the server over the pinned CA and tries the keychain, and prints PASS or FAIL for each.

## How it behaves
- **Finding the server:** addresses you typed by hand (remembered in `~/.config/local.pero.desktop/settings.json`), then `writer.local:8443`. The first that answers `/health` over the pinned CA is used. If none does, the sign-in screen asks for an address. When Tailscale is used, its name becomes one more entry in `src/settings.rs`.
- **The token:** you paste it once; the app checks it with the server and stores it in the keychain (Secret Service: GNOME Keyring or KWallet). On the next start the app signs in again by itself. "Sign out" removes it from the keychain. If the server says the token was revoked, the app forgets it. The web page never sees the token.
- **What the page may ask the app to do:** only `GET/POST/PUT/PATCH/DELETE` on `/health` and `/api/v1/...` of the found server (`src/rules.rs`). No other host, no headers, no files, no shell.
- **Trust:** the CA is copied into the binary at build time from `PERO_CA_FILE` (never from the repository or the system trust store). When the CA is renewed (2031), rebuild.

## Dependencies
The app is its own Cargo workspace (about 500 crates, mostly Tauri's). `desktop/deny.toml` holds the few approved exceptions by crate name (MPL-2.0 for five crates, CDLA-Permissive-2.0 for one) and the advisory warnings that come with Tauri's Linux backend; see `docs/DECISIONS.md`.
