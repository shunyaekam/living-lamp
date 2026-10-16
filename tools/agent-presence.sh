#!/bin/sh
# Show what an agent is doing on the lamp, through the organism. Safe to call from hooks: it never fails the caller.
#   agent-presence.sh working | done | needs-you | error | idle
command -v living-lamp >/dev/null 2>&1 || exit 0
case "${1:-}" in
  working|done|needs-you|error|idle) living-lamp send "agent $1" >/dev/null 2>&1 ;;
  *) echo "usage: agent-presence.sh working|done|needs-you|error|idle" >&2; exit 2 ;;
esac
exit 0
