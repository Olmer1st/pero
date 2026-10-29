# Client deploy: the web page on the server

Turns on the web client that turn 3 part 2 added: the server serves the built page at `https://writer.local:8443/`. The API behind it still needs a device token; the page itself is open to anyone who can reach the server (it holds no manuscript data).

## What changes on the server
| What | Change |
|---|---|
| Binary | new root-owned copy in `/opt/pero/server/<sha12>/`, `current` link switched |
| Web files | `/opt/pero/server/<sha12>/web/` (root-owned, read-only for the service; every file re-checked against `web.sha256`) |
| `/etc/pero/server.toml` | one line added: `web_dir = "/opt/pero/server/current/web"` |
| Backup | turn 2 settings and binary link in `/var/lib/pero/turn2-backup` |

Unchanged: certificate, unit, data folder, firewall, open ports (22, 8443, 9090), the LLM and LanguageTool services, all registered devices and books.

## Steps
| Step | Script | Where | What it does |
|---|---|---|---|
| 1 | `00-prepare.sh` | laptop | Checks and builds the client (`npm ci`, type check, tests, build, audit), builds the release server, assembles `target/deploy-client/`. Touches nothing on the server |
| 2 | copy and grant | laptop, then server | Copy `bundle/` to `~/deploy-client` and `payload/` to `~/pero-deploy-client` on the server; the owner runs `sudo ~/grant-sudo.sh ~/deploy-client <owner>` (checks `SHA256SUMS`, installs three root-owned scripts, one sudoers line each) |
| 3 | `10-install.sh` | server, root | Re-checks the client files, installs, adds `web_dir`, restarts, checks health, the page and that the API is still closed; puts turn 2 back by itself if the service is not healthy |
| 4 | `20-smoke.sh` | server, root | Read-only checks: page, headers (Content-Security-Policy, nosniff, no-store), assets, path tricks refused, API closed, web files root-owned and not writable by the service |
| 5 | `verify-from-client.sh <alias>` | laptop | The same from the network, with the CA, plus: no sudo grant left, three services active |
| 6 | remove the grant | server, owner | `sudo rm /etc/sudoers.d/pero-deploy-client` on the **server**, then `sudo -n -l` there shows nothing |

`30-rollback.sh` turns the page off again (turn 2 binary link and settings). Data is never touched.

## Then, as the owner
1. Register a device on the server if you have not: `sudo -u pero /opt/pero/server/current/pero-server device add "NAME" --config /etc/pero/server.toml` (token shown once; keep it in your password manager).
2. Import the CA (`~/.pero-ca/ca.crt`) into your browser once (Firefox: Settings, Privacy & Security, Certificates, Authorities, Import, trust for websites).
3. Open `https://writer.local:8443/`, paste the token, choose "Import chapters", pick your chapter files, and look for your book in the list.
