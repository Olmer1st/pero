# Part 8: Rust server skeleton with HTTPS

`pero-server` (Rust, axum, rustls) listens on `0.0.0.0:8443`, TLS 1.3 only. The only endpoint so far is `GET /health`:
status, version, uptime and up/down plus latency of the LLM and LanguageTool services. No login yet (a later turn); the response carries no secrets or manuscript data.

## Certificate: private home CA
- `05-make-ca.sh` (laptop) creates a CA in `~/.pero-ca` (EC P-256, 5 years). Its key never leaves the laptop. It is name-constrained to `writer.local` and the home subnet.
- The server creates its own key (`/etc/pero/tls/server.key`, user `pero`, mode 0600) and a certificate request; the laptop signs it for 365 days.
- The owner imports the public `ca.crt` into the client once (Firefox: Settings → Privacy & Security → Certificates → Authorities → Import, trust for websites).
- Renewal: before the server certificate expires (yearly), repeat steps 2, 3 and 5. Back up `~/.pero-ca` somewhere safe and offline.

| Step | Script | Where | What it does |
|---|---|---|---|
| 1 | `05-make-ca.sh` | laptop | Creates the name-constrained CA |
| 2 | `10-tls-csr.sh` | server, root | Creates the server key if absent and a certificate request |
| 3 | `20-sign-csr.sh <csr> <out.crt> <ip>` | laptop | Signs the request; names are fixed by the script |
| 4 | `25-build.sh` | laptop | Release build from the committed lockfile, prints SHA-256 |
| 5 | `30-install-server.sh` | server, root | Checks chain, key match, name and validity; installs; starts `pero-server` |
| 6 | `90-report.sh` | server, root | No-secrets report: listening sockets, firewall, certificate, sandbox score |
| 7 | `verify-from-client.sh <alias>` | laptop | Health over HTTPS with the CA; refused without CA, with TLS 1.2 and with plain HTTP |

Settings: `/etc/pero/server.toml` (example in `infra/config/`). Binary: `/opt/pero/server/<sha12>/pero-server`, root-owned, `current` symlink.
Restart: `sudo systemctl restart pero-server`. Logs: `sudo journalctl -u pero-server`.
Rollback: `sudo systemctl disable --now pero-server && sudo rm /etc/systemd/system/pero-server.service`.
