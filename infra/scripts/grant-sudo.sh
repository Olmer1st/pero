#!/usr/bin/env bash
# Lets the owner run EXACTLY the commands listed in a bundle's sudo-allow.txt as root without a password.
# - Copies the bundle to a root-only staging dir first, verifies SHA256SUMS there, then installs the scripts
#   root-owned under /opt/pero-provision/<bundle>/ (the owner cannot edit them afterwards).
# - Writes /etc/sudoers.d/pero-<bundle>: one explicit line per command, no wildcards, no extra arguments.
# Usage: sudo ./grant-sudo.sh <bundle-dir> <owner-username>
# Remove again: sudo rm /etc/sudoers.d/pero-<bundle>
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "Run with sudo." >&2; exit 1; }
SRC="$(readlink -f "${1:?usage: sudo $0 <bundle-dir> <owner>}")"
OWNER="${2:?usage: sudo $0 <bundle-dir> <owner>}"
id "$OWNER" >/dev/null
NAME="$(basename "$SRC")"
[[ "$NAME" =~ ^[A-Za-z0-9_-]+$ ]] || { echo "Bad bundle name: $NAME" >&2; exit 1; }
DEST="/opt/pero-provision/$NAME"

STAGE="$(mktemp -d)"; trap 'rm -rf "$STAGE"' EXIT
cp "$SRC"/*.sh "$SRC/sudo-allow.txt" "$SRC/SHA256SUMS" "$STAGE/"
(cd "$STAGE" && sha256sum -c SHA256SUMS)

install -d -m 0755 -o root -g root /opt/pero-provision "$DEST"
for f in "$STAGE"/*.sh; do install -m 0755 -o root -g root "$f" "$DEST/$(basename "$f")"; done
install -m 0644 -o root -g root "$STAGE/sudo-allow.txt" "$DEST/sudo-allow.txt"

RULES="$STAGE/rules"
{
  echo "# Pero provisioning bundle $NAME; remove with: rm /etc/sudoers.d/pero-$NAME"
  while IFS= read -r line; do
    case "$line" in ''|\#*) continue ;; esac
    [[ "$line" == *" "* ]] || line="$line \"\""      # no arguments allowed unless listed
    echo "$OWNER ALL=(root) NOPASSWD: $DEST/$line"
  done < "$STAGE/sudo-allow.txt"
} > "$RULES"
visudo -cf "$RULES"
install -m 0440 -o root -g root "$RULES" "/etc/sudoers.d/pero-$NAME"

echo "Installed to $DEST. Allowed without password:"
sed -n 's/^[^#].*NOPASSWD: /  /p' "/etc/sudoers.d/pero-$NAME"
