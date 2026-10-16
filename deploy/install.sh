#!/bin/sh
# Install the creature (lampbus-life) as a service for the current user. Changes nothing outside your home directory.
#
#   ./deploy/install.sh [--dry-run] [--bin-dir DIR] [--uninstall]
#
#   --dry-run     print what would happen and change nothing
#   --bin-dir DIR where the built binaries are (default: ./target/release)
#   --uninstall   stop and remove the service (keeps the binaries)
set -eu

DRY=0 UNINSTALL=0 BINDIR=""
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) DRY=1 ;;
    --uninstall) UNINSTALL=1 ;;
    --bin-dir) shift; BINDIR=${1:?--bin-dir needs a directory} ;;
    -h|--help) sed -n '2,9p' "$0"; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

run() { if [ "$DRY" = 1 ]; then echo "+ $*"; else "$@"; fi; }
HERE=$(cd "$(dirname "$0")" && pwd)
OS=$(uname -s)

if [ "$UNINSTALL" = 1 ]; then
  case "$OS" in
    Linux) run systemctl --user disable --now lampbus-life 2>/dev/null || true
           run rm -f "$HOME/.config/systemd/user/lampbus-life.service"; run systemctl --user daemon-reload ;;
    Darwin) run launchctl bootout "gui/$(id -u)/com.lampbus.life" 2>/dev/null || true
            run rm -f "$HOME/Library/LaunchAgents/com.lampbus.life.plist" ;;
    *) echo "unsupported OS: $OS" >&2; exit 1 ;;
  esac
  echo "service removed; kept: ~/.local/bin/lampbus-life"
  exit 0
fi

[ -n "$BINDIR" ] || BINDIR="$HERE/../target/release"
[ -x "$BINDIR/lampbus-life" ] || { echo "no lampbus-life binary in $BINDIR; build with: cargo build --release -p lampbus-life" >&2; exit 1; }
run mkdir -p "$HOME/.local/bin"
run install -m 0755 "$BINDIR/lampbus-life" "$HOME/.local/bin/lampbus-life"

case "$OS" in
  Linux)
    run mkdir -p "$HOME/.config/systemd/user"
    run cp "$HERE/lampbus-life.service" "$HOME/.config/systemd/user/lampbus-life.service"
    run systemctl --user daemon-reload
    run systemctl --user enable --now lampbus-life
    echo "status:  systemctl --user status lampbus-life" ;;
  Darwin)
    plist=com.lampbus.life.plist
    run mkdir -p "$HOME/Library/LaunchAgents" "$HOME/Library/Logs"
    if [ "$DRY" = 1 ]; then echo "+ install $plist into ~/Library/LaunchAgents (with __HOME__ replaced)"; else
      sed "s|__HOME__|$HOME|g" "$HERE/$plist" > "$HOME/Library/LaunchAgents/$plist"
      launchctl bootout "gui/$(id -u)/${plist%.plist}" 2>/dev/null || true
      # bootout returns before the old service has gone; bootstrap then fails with "Input/output error". Wait for it.
      for _ in 1 2 3 4 5; do launchctl print "gui/$(id -u)/${plist%.plist}" >/dev/null 2>&1 || break; sleep 1; done
      launchctl bootstrap "gui/$(id -u)" "$HOME/Library/LaunchAgents/$plist" || {
        sleep 2; launchctl bootstrap "gui/$(id -u)" "$HOME/Library/LaunchAgents/$plist"; }
    fi
    echo "logs:    tail -f ~/Library/Logs/lampbus-life.log" ;;
  *) echo "unsupported OS: $OS" >&2; exit 1 ;;
esac
