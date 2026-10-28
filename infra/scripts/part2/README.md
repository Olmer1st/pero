# Part 2: base hardening

Run order. Each step is separate; stop and check after each one. All steps need `sudo`
(run them in Cockpit's Terminal), except the client check.

| Step | Script | What it does |
|---|---|---|
| 1 | `10-updates.sh` | Applies updates; keeps automatic security updates on |
| 2 | `20-service-user.sh` | Creates system user `pero` (no login), `/var/lib/pero`, `/etc/pero` (0700) |
| 3 | `30-ssh.sh <owner>` | SSH key-only, no root login (drop-in file) |
| 4 | `40-firewall.sh <home-subnet-cidr>` | ufw default-deny; 22, 9090, 8443 from the home subnet only |
| 5 | `50-power.sh` | Ignore lid switch; mask sleep targets |
| 6 | `90-report.sh` | Writes a no-secrets status report for remote checking |
| 7 | `verify-from-client.sh <host>` | Run from another machine: password and root login refused, key works, ports |

## Safety
- Keep a Cockpit session open throughout. Never close it before key login is confirmed from a new session.
- Step 3 refuses to run if the owner has no authorized key; it validates the config and reverts itself on error.
- Step 4 arms a 5-minute timer that switches the firewall off again. Confirm from another machine, then run
  `sudo systemctl stop pero-ufw-rollback.timer`. Forgetting this leaves the firewall off: step 6 shows it.
- Verify what you run: `sha256sum -c SHA256SUMS` in the folder you copied to the server.

## Rollback
- SSH: `sudo rm /etc/ssh/sshd_config.d/10-pero-hardening.conf && sudo systemctl reload ssh`
- Firewall: `sudo ufw disable`
- Power: printed at the end of step 5
- Service user: harmless; leave it.

## Notes
- IPv6 inbound is denied by default (the allow rules are IPv4 subnet rules).
- Later users of SSH (backup laptop, turn 9) need their own authorized keys.
- Not covered here: Wi-Fi power saving, disabling bluetooth/ModemManager (suggested follow-ups).
