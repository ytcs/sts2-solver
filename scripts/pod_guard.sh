#!/bin/bash
# budget backstop (started by pod_job.sh when DEADLINE, epoch s, is set): terminate this pod at DEADLINE or 40 min after DONE / FAILED
export $(tr "\0" "\n" < /proc/1/environ | grep -E "^RUNPOD_(API_KEY|POD_ID)=" | xargs)
while true; do
  now=$(date +%s); why=""
  [ $now -ge $DEADLINE ] && why="deadline"
  for m in $(dirname "$0")/../target/job/DONE $(dirname "$0")/../target/job/FAILED; do
    [ -f $m ] && [ $(( now - $(stat -c %Y $m) )) -ge 2400 ] && why="idle after $(basename $m)"
  done
  if [ -n "$why" ]; then
    echo "$(date -u +%T) terminate: $why" >> "$(dirname "$0")/../target/job/guard.log"
    curl -s -H "Content-Type: application/json" -H "Authorization: Bearer $RUNPOD_API_KEY" https://api.runpod.io/graphql \
      -d "{\"query\":\"mutation { podTerminate(input:{podId:\\\"$RUNPOD_POD_ID\\\"}) }\"}" >> "$(dirname "$0")/../target/job/guard.log" 2>&1
    sleep 300
  fi
  sleep 60
done
