# Part 9: security baseline

Produces `docs/security/baseline.md`: what listens, what the firewall allows, how SSH is set up, which services run as whom, every package added, scan results and accepted exceptions.
Nothing is installed or changed on the server by this part.

| Step | Script | Where | What it does |
|---|---|---|---|
| 1 | `90-collect.sh` | server, root (sudo) | Read-only facts into `/var/tmp/pero-part9-baseline.txt`: sockets with owners, ufw, effective SSH settings, accounts, sudoers drop-ins, services with sandbox scores, file permissions, SHA-256 of programs and model, apt sources, explicitly installed packages, pending updates, setuid list. No keys, no environment files, no manuscript data |
| 2 | `verify-from-client.sh <alias>` | laptop | Scans all 65535 TCP ports from the network (only 22, 8443, 9090 may answer), checks password and root SSH login are refused and that no passwordless sudo grant is left; prints the server facts |
| 3 | repo checks | laptop | `cargo audit`, `cargo deny check`, gitleaks over the full Git history |

Accepted exception: Cockpit (9090) stays open to the home subnet only (owner decision, part 2).
