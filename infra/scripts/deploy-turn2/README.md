# Turn 2 deploy: manuscript API on the server

Puts the turn 2 server on the home server: the manuscript store, story graph, canon ledger and device login. Until `data_dir` is set the new binary behaves like turn 1 (only `/health`).

## What changes on the server
| What | Change |
|---|---|
| Binary | new root-owned copy in `/opt/pero/server/<sha12>/`, `current` symlink switched |
| Certificate | new server certificate for the reserved address; **same private key**, signed on the laptop (CA key never leaves it) |
| `/etc/pero/server.toml` | one line added: `data_dir = "/var/lib/pero/data"` |
| `pero-server.service` | one line added: `ReadWritePaths=/var/lib/pero/data` (everything else stays read-only under `ProtectSystem=strict`) |
| New folder | `/var/lib/pero/data`, owner `pero`, mode 0700 (SQLite file and one Git repository per book) |
| Backup | turn 1 unit, settings, certificate and binary link in `/var/lib/pero/turn1-backup` |

Nothing else changes: no new packages, no new open ports (still 22, 8443, 9090), no change to the firewall, the LLM or LanguageTool.

## Steps
| Step | Script | Where | What it does |
|---|---|---|---|
| 1 | `00-prepare.sh <server-ip>` | laptop | Release build, new certificate, bundle in `target/deploy-turn2/`. Touches nothing on the server |
| 2 | copy and grant | laptop, then server | Copy `bundle/` and `payload/` to the server home; owner runs `grant-sudo.sh` (checks `SHA256SUMS`, installs the three scripts root-owned, writes a sudoers drop-in for exactly these commands) |
| 3 | `10-install.sh <server-ip>` | server, root | Checks the certificate (chain, key match, names, 30+ days), installs everything above, restarts, checks `/health`. If the service does not come up healthy it puts turn 1 back by itself |
| 4 | `20-smoke.sh` | server, root | Smoke test of the real service and sandbox (see below), then removes the test data |
| 5 | `verify-from-client.sh <alias>` | laptop | Checks from the network, with the CA: certificate also valid for the address, API refuses without a token |
| 6 | remove the grant | server, owner | `sudo rm /etc/sudoers.d/pero-deploy-turn2`, then `sudo -n -l` **on the server** shows nothing |
| 7 | register your devices | server, owner | `sudo -u pero /opt/pero/server/current/pero-server device add "NAME" --config /etc/pero/server.toml` prints a token once; keep it in your password manager. The assistant never sees it |

`30-rollback.sh` puts the turn 1 setup back (binary, settings, unit, certificate) and keeps the data folder.

## The smoke test
It refuses to run unless the data folder is empty, so it can never touch real data. It registers a throw-away device `smoke-test` (the token stays in the script's memory), then over HTTPS on the server itself: checks that requests without a token and with a wrong token get 401; creates a book, a chapter and a scene; saves Russian text; reads it back and exports it; checks that Git, running inside the service's sandbox, made commits authored `smoke-test` and left the tree clean; revokes the device and checks the token is refused at once; checks the token is not in the log. Finally it stops the service, removes the database and the one test book, and starts the service again (empty). Imports of your real book happen later, from the client.

## Notes
- The certificate renewal in 2027 (`infra/scripts/part8/README.md`) now needs this deploy's settings: `30-install-server.sh` from part 8 rewrites `server.toml` and the unit **without** `data_dir` and `ReadWritePaths`. Use `10-install.sh` for renewals too (or fix part 8's script first).
- `/health` stays open (issue #12). The API answers 401 to everything without a valid token.
- Rollback of the data: there is nothing to roll back before the first real import; after that the data folder is the book, back it up (a Git repository per book).
