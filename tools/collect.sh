#!/usr/bin/env bash
# Runs `rl/exit.py collect` in fresh processes until every chunk is saved (it exits with 3 while chunks remain). A watchdog kill (exit 2) is retried
# from the saved parts up to 3 times (transient machine load slows a chunk now and then; a real hang repeats); any other exit code stops.
# Usage: tools/collect.sh <rl/exit.py collect arguments>
retries=0
while true; do
  .venv/Scripts/python.exe rl/exit.py collect "$@"
  rc=$?
  if [ $rc -eq 2 ] && [ $retries -lt 3 ]; then
    retries=$((retries + 1)); echo "watchdog kill: retry $retries of 3"
    tasklist //FO CSV //NH 2>/dev/null | sort -t, -k5 -r | head -5   # what else was running
    continue
  fi
  [ $rc -eq 3 ] || exit $rc
done
