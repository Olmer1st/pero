# Part 7: LanguageTool service

LanguageTool 6.8 (LGPL-2.1), official release from Maven Central, SHA-256 pinned in the script. No n-gram data, no premium features, no telemetry.
The server runs with languages given explicitly by the caller (`language=ru-RU` or `en-US`); automatic language detection is not used.

| Step | Script | Root | What it does |
|---|---|---|---|
| 1 | `10-install-languagetool.sh` | yes (sudo) | Downloads and verifies the zip, installs it root-owned in `/opt/pero/languagetool/6.8`, writes `/etc/pero/languagetool.properties` and the sandboxed unit `pero-languagetool.service`, starts it |
| 2 | `90-report.sh` | yes | No-secrets status report |
| 3 | `verify-from-client.sh <alias>` | no | From the laptop: port 8010 unreachable from the network, loopback bind, enabled at boot, seeded Russian and English errors found |
| 4 | reboot, then `verify-from-client.sh` again | owner | Survives reboot (done together with the part 6 reboot test) |

Service: user `pero`, `127.0.0.1:8010`, 2 GB Java heap cap. About 0.9 GB resident after checking a 51,000-character chapter, which took about 4 seconds.
Settings: `/etc/pero/languagetool.properties` (maximum text length and check time). Restart: `sudo systemctl restart pero-languagetool`.
Logs: `sudo journalctl -u pero-languagetool` (lengths and timings only, no text).
Rollback: `sudo systemctl disable --now pero-languagetool && sudo rm /etc/systemd/system/pero-languagetool.service`.
