#!/usr/bin/env bash
# Usage: tools/collect.sh <rl/exit.py collect arguments>  (reruns on exit 3, retries a watchdog kill up to 3 times)
retries=0
PY=.venv/Scripts/python.exe; [ -x "$PY" ] || PY=.venv/bin/python
while true; do
  "$PY" rl/exit.py collect "$@"
  rc=$?
  if [ $rc -eq 2 ] && [ $retries -lt 3 ]; then
    retries=$((retries + 1)); echo "watchdog kill: retry $retries of 3"
    tasklist //FO CSV //NH 2>/dev/null | sort -t, -k5 -r | head -5
    continue
  fi
  [ $rc -eq 3 ] || exit $rc
done
