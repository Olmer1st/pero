# Part 3: name resolution (writer.local)

Tailscale and remote access are deferred (owner decision, 2026-10-26). This part only gives the server
a stable name on the home network. Run in Cockpit's Terminal with `sudo`, one step at a time.

| Step | Script | What it does |
|---|---|---|
| 1 | `10-avahi.sh <home-subnet-cidr>` | Installs avahi, announces the host as `writer.local` (IPv4, home network only), opens UDP 5353 from the home subnet |
| 2 | `20-listener-test.sh [seconds]` | Temporary 8443 listener (user `pero`) so the client can test reachability; stops itself |
| 3 | `90-report.sh` | No-secrets status report for remote checking |
| 4 | `verify-from-client.sh <host>` | From the dev laptop, at home |

## Notes
- Nothing new is exposed: the firewall already allows 8443 from the home subnet only; no listener remains after step 2.
- The address is a DHCP lease on Wi-Fi; `writer.local` hides changes, a router reservation is still advisable.
- Verify what you run: `sha256sum -c SHA256SUMS`.

## Rollback
- Avahi: `sudo systemctl disable --now avahi-daemon`; original config is kept as `/etc/avahi/avahi-daemon.conf.orig`
