#!/bin/sh
# Create or remove a throwaway Rex session to drive, so the user's live sessions are never touched.
# Usage: scratch-session.sh new [label]   -> prints the session label
#        scratch-session.sh capture LABEL -> prints its terminal screen
#        scratch-session.sh kill LABEL    -> kills it (refuses labels not starting verify-rexling-)
set -eu
case "${1:-}" in
new)
  label="${2:-verify-rexling-$(date +%H%M%S)}"
  rex new "$label" --cwd /tmp --keep-open >/dev/null
  echo "$label"
  ;;
capture)
  # Name the block explicitly: without -b, rex capture prefers $REX_BLOCK (the caller's own block).
  block=$(rex block ls -s "$2" | awk 'NR==2{print $2}')
  rex capture -s "$2" -b "$block" --trim
  ;;
kill)
  case "$2" in verify-rexling-*) rex kill "$2" ;; *) echo "refusing to kill non-scratch session '$2'" >&2; exit 1 ;; esac
  ;;
*) echo "usage: $0 new [label] | capture LABEL | kill LABEL" >&2; exit 2 ;;
esac
