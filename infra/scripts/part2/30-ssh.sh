#!/usr/bin/env bash
# Part 2, step 3: SSH key-only login, no root login.
# Usage: sudo ./30-ssh.sh <owner-username>
# Keep your current Cockpit/SSH session open until key login is confirmed from a NEW session.
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo." >&2; exit 1; }
OWNER="${1:?usage: sudo $0 <owner-username>}"
HOME_DIR="$(getent passwd "$OWNER" | cut -d: -f6)"
[ -n "$HOME_DIR" ] || { echo "No such user: $OWNER" >&2; exit 1; }
[ -s "$HOME_DIR/.ssh/authorized_keys" ] || {
  echo "$OWNER has no authorized key; refusing to disable password login." >&2; exit 1; }

CONF=/etc/ssh/sshd_config.d/10-pero-hardening.conf
cat > "$CONF" <<'CONF'
PermitRootLogin no
PasswordAuthentication no
KbdInteractiveAuthentication no
PubkeyAuthentication yes
X11Forwarding no
MaxAuthTries 3
LoginGraceTime 30
CONF

if ! sshd -t; then
  rm -f "$CONF"
  echo "sshd config test failed; change reverted." >&2
  exit 1
fi
systemctl reload ssh    # reload keeps existing sessions alive

sshd -T | grep -E '^(permitrootlogin|passwordauthentication|kbdinteractiveauthentication|pubkeyauthentication|x11forwarding|maxauthtries) '
echo "Step 3 done. Now open a NEW SSH session with your key to confirm it still works."
echo "Rollback: sudo rm $CONF && sudo systemctl reload ssh"
