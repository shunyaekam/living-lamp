#!/bin/sh
# Install the organism (living-lamp) and/or the older creature (lampbus-life) as services for the current user.
# Changes nothing outside your home directory. Review it, or run with --dry-run first.
#
#   ./deploy/install.sh [--organism] [--creature] [--dry-run] [--bin-dir DIR] [--uninstall]
#
#   --organism    install living-lamp (the default when neither is given)
#   --creature    install lampbus-life, the standalone creature that talks to lampbus without owning it
#   --dry-run     print what would happen and change nothing
#   --bin-dir DIR where the built binaries are (default: ./target/release)
#   --uninstall   stop and remove the chosen services (keeps the binaries)
set -eu

ORG=0 CRE=0 DRY=0 UNINSTALL=0 BINDIR=""
while [ $# -gt 0 ]; do
  case "$1" in
    --organism) ORG=1 ;;
    --creature) CRE=1 ;;
    --dry-run) DRY=1 ;;
    --uninstall) UNINSTALL=1 ;;
    --bin-dir) shift; BINDIR=${1:?--bin-dir needs a directory} ;;
    -h|--help) sed -n '2,12p' "$0"; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done
[ "$ORG" = 1 ] || [ "$CRE" = 1 ] || ORG=1

run() { if [ "$DRY" = 1 ]; then echo "+ $*"; else "$@"; fi; }
HERE=$(cd "$(dirname "$0")" && pwd)
OS=$(uname -s)
[ -n "$BINDIR" ] || BINDIR="$HERE/../target/release"

# name:binary:launchd-label:plist:systemd-unit
SERVICES=""
[ "$ORG" = 0 ] || SERVICES="$SERVICES living-lamp:living-lamp:com.living-lamp.organism:com.living-lamp.organism.plist:living-lamp"
[ "$CRE" = 0 ] || SERVICES="$SERVICES lampbus-life:lampbus-life:com.lampbus.life:com.lampbus.life.plist:lampbus-life"

if [ "$UNINSTALL" = 1 ]; then
  for s in $SERVICES; do
    label=$(echo "$s" | cut -d: -f3); plist=$(echo "$s" | cut -d: -f4); unit=$(echo "$s" | cut -d: -f5)
    case "$OS" in
      Linux) run systemctl --user disable --now "$unit" 2>/dev/null || true
             run rm -f "$HOME/.config/systemd/user/$unit.service" ;;
      Darwin) run launchctl bootout "gui/$(id -u)/$label" 2>/dev/null || true
              run rm -f "$HOME/Library/LaunchAgents/$plist" ;;
      *) echo "unsupported OS: $OS" >&2; exit 1 ;;
    esac
  done
  [ "$OS" != Linux ] || run systemctl --user daemon-reload
  echo "services removed; kept: the binaries in ~/.local/bin"
  exit 0
fi

for s in $SERVICES; do
  bin=$(echo "$s" | cut -d: -f2)
  [ -x "$BINDIR/$bin" ] || { echo "no $bin binary in $BINDIR; build with: cargo build --release" >&2; exit 1; }
done
run mkdir -p "$HOME/.local/bin"
for s in $SERVICES; do
  bin=$(echo "$s" | cut -d: -f2)
  run install -m 0755 "$BINDIR/$bin" "$HOME/.local/bin/$bin"
done

for s in $SERVICES; do
  label=$(echo "$s" | cut -d: -f3); plist=$(echo "$s" | cut -d: -f4); unit=$(echo "$s" | cut -d: -f5)
  case "$OS" in
    Linux)
      run mkdir -p "$HOME/.config/systemd/user"
      run cp "$HERE/$unit.service" "$HOME/.config/systemd/user/$unit.service"
      run systemctl --user daemon-reload
      run systemctl --user enable --now "$unit"
      echo "status:  systemctl --user status $unit" ;;
    Darwin)
      run mkdir -p "$HOME/Library/LaunchAgents" "$HOME/Library/Logs"
      if [ "$DRY" = 1 ]; then echo "+ install $plist into ~/Library/LaunchAgents (with __HOME__ replaced)"; else
        sed "s|__HOME__|$HOME|g" "$HERE/$plist" > "$HOME/Library/LaunchAgents/$plist"
        launchctl bootout "gui/$(id -u)/$label" 2>/dev/null || true
        # bootout returns before the old service has gone; bootstrap then fails with "Input/output error". Wait for it.
        for _ in 1 2 3 4 5; do launchctl print "gui/$(id -u)/$label" >/dev/null 2>&1 || break; sleep 1; done
        launchctl bootstrap "gui/$(id -u)" "$HOME/Library/LaunchAgents/$plist" || {
          sleep 2; launchctl bootstrap "gui/$(id -u)" "$HOME/Library/LaunchAgents/$plist"; }
      fi
      echo "logs:    tail -f ~/Library/Logs/living-lamp.log (organism) or lampbus-life.log (creature)" ;;
    *) echo "unsupported OS: $OS" >&2; exit 1 ;;
  esac
done
