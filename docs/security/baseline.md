# Security baseline — turn 1

Measured on 2026-10-28 (server Debian 13 "trixie", kernel 6.12.113, up 2 days; dev laptop for the scans).
Method: `infra/scripts/part9/90-collect.sh` (read-only, on the server) and `infra/scripts/part9/verify-from-client.sh` (from another machine on the home network). Server host addresses are left out on purpose; only the home subnet `10.0.0.0/24` is named.

**Result: from the network only SSH (22), the Pero server (8443) and Cockpit (9090) answer, and all three accept the home subnet only.** Cockpit is an accepted exception to the engineering rule "only 8443" (see "Accepted exceptions").

## 1. Ports and bind addresses

| Port | Service | Bound to | User | Reachable from |
|---|---|---|---|---|
| 22/tcp | sshd | all addresses (IPv4 and IPv6) | root (privilege separation) | home subnet via ufw |
| 8443/tcp | `pero-server` (TLS 1.3 only) | all addresses | `pero` | home subnet via ufw |
| 9090/tcp | Cockpit (`cockpit-tls`, socket-activated) | all addresses | root / cockpit-* | home subnet via ufw |
| 8080/tcp | `pero-llm` (llama.cpp) | 127.0.0.1 | `pero` | this machine only |
| 8010/tcp | `pero-languagetool` (Java) | 127.0.0.1 | `pero` | this machine only |
| 5353/udp | avahi-daemon (mDNS, `writer.local`) | all addresses | avahi | home subnet via ufw |
| 68/udp, 546/udp | dhcpcd (DHCP client, IPv4 and IPv6) | client ports | root | not a service |

Checks from the dev laptop over the home network:
- Full TCP scan, ports 1–65535 on the server's IPv4 address: only **22, 8443, 9090** answered.
- IPv6: the server has global IPv6 addresses; ports 22, 8443, 9090, 8080 and 8010 on its global address all timed out (ufw has `IPV6=yes` and a default-deny policy with no IPv6 allow rules).
- UDP is not scanned; the UDP sockets above come from `ss` on the server and the only inbound rule is 5353/udp from the subnet.

## 2. Firewall (ufw)

Active, logging low, default deny incoming, allow outgoing, routing disabled. Rules (all from `10.0.0.0/24` only): 22/tcp ssh, 9090/tcp Cockpit, 8443/tcp Pero server, 5353/udp mDNS. No rules for 8080 or 8010.
Kernel: `net.ipv4.ip_forward = 0`, `accept_redirects = 0`. `rp_filter = 0` and IPv6 `accept_ra = 1` are Debian defaults; with no routing and IPv6 closed by ufw they carry no exposure, but see section 9.

## 3. SSH

Effective settings (`sshd -T`, drop-in `/etc/ssh/sshd_config.d/10-pero-hardening.conf`): `permitrootlogin no`, `passwordauthentication no`, `kbdinteractiveauthentication no`, `pubkeyauthentication yes`, `permitemptypasswords no`, `x11forwarding no`, `maxauthtries 3`, `logingracetime 30`. Verified from the laptop: password login refused (`Permission denied (publickey)`), root login refused.

## 4. Accounts, privileges and services

| Account | Role |
|---|---|
| `olmer` (uid 1000) | owner, member of group `sudo`; sudo asks for a password |
| `root` | no SSH login |
| `pero` (uid 101) | service user: shell `/usr/sbin/nologin`, home `/var/lib/pero`, password locked |
| `cockpit-*` | Cockpit helper accounts, `nologin` |

Sudo: no passwordless rules in normal operation. The assistant's one-time grants (`infra/scripts/grant-sudo.sh`, one per part) were removed by the owner after each part; the script copies they installed remain in `/opt/pero-provision/pero-part*` (root-owned, inert without a grant). The last check that no grant is installed is recorded in section 8.

Pero services (all enabled at boot, active, 0 restarts, running as `pero`, `systemd-analyze security` exposure level): `pero-llm` 1.7, `pero-languagetool` 1.7, `pero-server` 1.7 (all "OK").
Permissions: `/etc/pero` is `pero:pero` 0700; `/etc/pero/tls/server.key` is `pero` 0600; settings files are `root:pero` 0640; everything under `/opt/pero` is root-owned and nothing there is world-writable (symlinks excluded); the models directory is `pero:pero` 0750. No failed systemd units. Setuid binaries: the 15 standard Debian ones (`su`, `sudo`, `passwd`, `mount`, `ssh-keysign`, `pppd`, …), no custom ones.
Other enabled services (Debian defaults for a laptop plus ours): ssh, cron/anacron, NetworkManager, wpa_supplicant, networking, avahi-daemon, unattended-upgrades, ufw, apparmor, bluetooth, ModemManager, udisks2, systemd-timesyncd, cockpit.socket.

## 5. Software added, with sources and versions

**Debian packages (official repositories only: `trixie`, `trixie-updates`, `trixie-security`, components `main` and `non-free-firmware`; no third-party repositories, no packages installed outside a repository):**

| When | Package (version) | Why |
|---|---|---|
| 2026-10-25 (OS installation) | Debian installer set: busybox, zstd, initramfs-tools, linux-image-amd64, pciutils, usbutils, wireless-tools, wpasupplicant, console-setup, keyboard-configuration, laptop-detect, grub-efi-amd64, intel-microcode, installation-report, tasksel, task-english, task-ssh-server, task-laptop, reportbug, … | base system |
| 2026-10-25 (owner, before turn 1) | sudo 1.9.16p2-3+deb13u2, cockpit 337-1+deb13u2, unattended-upgrades 2.12, curl 8.14.1-2+deb13u7 | admin tooling |
| 2026-10-26 (part 2) | ufw 0.36.2-9; locales 2.41-12+deb13u4 | firewall |
| 2026-10-26 (part 3) | avahi-daemon 0.8-16 (no recommends) | `writer.local` |
| 2026-10-27 (part 4) | build-essential 12.12, cmake 3.31.6-2, git 1:2.47.3-0+deb13u1, ccache 4.11.2-2, pkg-config 1.8.1-4, libcurl4-openssl-dev 8.14.1-2+deb13u7, openjdk-21-jre-headless 21.0.12.1+1-1~deb13u1 | build llama.cpp, run LanguageTool |

`popularity-contest` (Debian's package statistics reporter, installed by the installer) is **not installed now** and has no configuration left: no telemetry from the OS.

**Software from outside apt (each pinned and checked when installed):**

| Component | Version | Source | SHA-256 on the server |
|---|---|---|---|
| llama.cpp `llama-server` | tag b11796, commit 0a1ef30 | official GitHub project, built on the server for its CPU | `61e82a65…d023a9` (llama-server) |
| Gemma 4 26B-A4B-it Q4_K_M | UD-Q4_K_M | Hugging Face (quantizer release), SHA-256 verified | `f2c28b3d…93f` |
| LanguageTool | 6.8 | Maven Central, SHA-256 pinned, maintainer GPG signature checked | `07caad9d…3b2e` (languagetool-server.jar) |
| `pero-server` | 0.1.0 | built from this repository on the dev laptop (`--locked`) | `50897270…f97` |

**Dev laptop only (not on the server):** Rust 1.99.1 (official rustup, installer hash pinned), cargo-audit 0.22.2, cargo-deny 0.20.2, gitleaks 8.30.1 (official release, SHA-256 verified).

**Rust dependencies of `pero-server`:** 8 direct crates (axum, tokio, rustls with the ring provider, tokio-rustls, rustls-pki-types, serde, serde_json, toml), 76 crates in total, licenses MIT, Apache-2.0, ISC, BSD-3-Clause, Unicode-3.0, Unlicense (alternative to MIT) only.

## 6. Audit and scan results

| Check | Result |
|---|---|
| `cargo audit` (1381 advisories, 76 crates) | no vulnerabilities |
| `cargo deny check` (advisories, bans, licenses, sources) | all four ok; warnings only: two versions of `windows-sys` (Windows-only, not built on Linux) and three allowed licenses not used |
| gitleaks over the full Git history (24 commits, all branches) | no leaks |
| `apt-get -s upgrade` on the server | 0 pending upgrades, none security; no reboot required |
| Automatic security updates | `unattended-upgrades` enabled and active |
| 12 unit tests, clippy `-D warnings`, `cargo fmt --check` | pass |
| TLS (part 8) | certificate trusted via the home CA; refused without the CA; TLS 1.2 refused; plain HTTP not served |

## 7. Secrets and certificates

- No credentials in the repository (secret scan above). The GitHub token stays in git's credential helper, scoped to this repository.
- Server TLS key: only on the server, `pero` 0600. The CA key `~/.pero-ca/ca.key`: only on the dev laptop, mode 0600, never on the server. **Back it up offline.**
- Home CA (name-constrained to `writer.local` and `10.0.0.0/24`) is valid to 2031-10-27, fingerprint SHA-256 `62:D8:0E:4B:04:BA:18:8B:13:9A:24:DE:1D:76:14:65:AF:CF:2D:82:A3:D4:65:5B:BB:D8:48:3E:9E:92:48:0E`. The server certificate expires 2027-10-28: **renew by 2027-09-28**.

## 8. Final grant check

After the owner removed the part 9 grant (2026-10-28 09:10), `verify-from-client.sh` passed all four checks again: only 22, 8443 and 9090 answer, SSH password and root logins are refused, and `sudo -n -l` on the server lists no passwordless rule. `/etc/sudoers.d` holds only Debian's `README`.
Note: the collector copy that ran on the server is the version before one small fix (it listed symlinks as world-writable because they show mode `rwxrwxrwx`). The repository script is corrected; a separate `find ... ! -type l` on the server confirmed that no real file under `/opt/pero`, `/etc/pero` or `/var/lib/pero` is world-writable.

## 9. Accepted exceptions, known gaps and recommendations

Accepted by the owner:
1. **Cockpit on 9090 stays open to the home subnet.** The engineering rule says only 8443 may listen on the network; SSH and Cockpit are the administration paths. Mitigation: ufw limits both to the subnet; SSH is key-only; Cockpit requires an account password. Revisit when Tailscale is added.

Known gaps (not changed in this turn):
2. `/health` on 8443 has no login; it reveals only up/down, version and uptime. Authentication comes with the client turn.
3. `pero-server` has no idle timeout after the TLS handshake (handshakes time out after 10 s; at most 256 connections).
4. The server uses Wi-Fi only; its address is a DHCP lease, not a reservation. A DHCP reservation or cable is advisable.
5. Reboot test for the three Pero services (and Wi-Fi reconnection) and the part 2 lid-close test are still pending the owner being at the machine.
6. Backups of the CA key and of `~/pero*` bundles are not automated (turn 9 covers book backups).

Recommendations for the owner to decide (no change made):
7. Disable unused services `bluetooth`, `ModemManager` and `udisks2` (no listening sockets, but unneeded code on a server), and confirm that only one of NetworkManager and `networking`/wpa_supplicant manages Wi-Fi.
8. Remove the inert copies `/opt/pero-provision/pero-part*` when no more parts need them.
9. Optional: set `rp_filter` to strict and `accept_ra` to 0 via a sysctl drop-in.
