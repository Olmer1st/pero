# Server inventory (turn 1, part 1)

Collected 2026-10-26, read-only, over SSH as the unprivileged owner account. No system change was made. Addresses are omitted on purpose: this repository is public.

## Findings

| Area | Finding |
|---|---|
| Hardware | Dell Latitude 5580 (BIOS 2024-09-05), on AC, battery full |
| OS | **Debian 13 "trixie"** (current stable), kernel 6.12 — not Ubuntu |
| CPU | Intel Core i7-7820HQ, 4 cores / 8 threads, max 3.9 GHz, 8 MiB L3. AVX, AVX2, FMA, F16C. **No AVX-512** |
| RAM | **31 GiB** usable (not 64 GB). Memory type and channels need root (`dmidecode`); not collected |
| Swap | 24 GiB swap partition on NVMe, unused |
| Disk | 466 GB NVMe SSD; root ext4 433 GB, 410 GB free (1% used) |
| Network | Wi-Fi only (`wlp2s0`, DHCP). Ethernet (`enp0s31f6`) present but down. IPv6 addresses assigned |
| Listening ports | TCP 22 (sshd) and TCP 9090 (Cockpit), both on all interfaces. Nothing on 8443 |
| Services | Enabled: NetworkManager, wpa_supplicant, ssh, cron, anacron, unattended-upgrades, bluetooth, ModemManager, udisks2, systemd-timesyncd. Running extras: cockpit, packagekit, polkit |
| Users | One interactive user (uid 1000) in the `sudo` group. No service user. `sudo` requires a password |
| SSH | Default Debian config: password login **enabled**, key login works (key added during this part). No hardening drop-ins |
| Firewall | None active (ufw not installed; nftables inactive) |
| Updates | No upgradable packages, no reboot pending, unattended-upgrades enabled, up 23 h |
| Lid / sleep | No logind overrides: default lid-close action is suspend; sleep targets not masked |
| Time | Clock synchronised; time zone America/Chicago |
| Installed tools | `curl`, `python3`. Missing: `git`, compiler, `make`, `cmake`, Java, Rust, Tailscale, avahi, ufw |

## Differences from the handoff's assumptions

1. **Debian, not Ubuntu.** The rules mention Ubuntu official repos; the equivalent here is Debian official repos. Package names are mostly the same; Tailscale publishes a signed Debian trixie repo.
2. **32 GB RAM, not 64 GB.** A Q4_K_M 35B-A3B model is about 20–22 GB, so it fits but leaves roughly 9–10 GB for the OS, LanguageTool, the NLP sidecar and the Rust server. The benchmark in part 5 must include memory headroom. Note that the Latitude 5580 tops out at 32 GB, so this may not be the intended 64 GB machine.
3. **Wi-Fi only.** An always-on server on Wi-Fi is less reliable and its address can change; a wired link or a DHCP reservation is advisable. mDNS (`writer.local`) will help but does not replace a stable address.
4. **Cockpit listens on 9090 on all interfaces.** The rules allow only the Rust server on the network. Cockpit needs a decision: restrict it by firewall to the home subnet and the Tailscale interface, bind it to localhost, or remove it after setup.

## Recommendation: keep, no reinstall

Debian 13 is current stable with full support, packages are up to date and the disk is empty, so reinstalling or upgrading the OS gains nothing. Proceed with hardening (part 2) on the existing install after the owner accepts the differences above.

Root actions cannot be run over SSH because `sudo` needs a password. Plan: each privileged step is a reviewed script in `infra/`, run by the owner in Cockpit's Terminal with `sudo`; results are then verified read-only over SSH.

## Not collected (needs root)

RAM type and speed, SMART disk health, effective `sshd -T` output, firewall rule listing.
