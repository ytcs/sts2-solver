#!/usr/bin/env bash
# Runs `rl/exit.py collect` in fresh processes until every chunk is saved (it exits with 3 while chunks remain); any other exit code stops.
# Usage: tools/collect.sh <rl/exit.py collect arguments>
while true; do
  .venv/Scripts/python.exe rl/exit.py collect "$@"
  rc=$?
  [ $rc -eq 3 ] || exit $rc
done
