#!/usr/bin/env bash
# Any GPU job on a fresh pod, from the repo root: nohup bash scripts/pod_job.sh >/dev/null 2>&1 &
# Builds, waits for $INPUTS/READY, runs $INPUTS/job.sh from the repo root; log and DONE / FAILED in target/job/.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p target/job
exec >>target/job/pod.log 2>&1
trap 'echo "line $LINENO: $BASH_COMMAND" >target/job/FAILED' ERR
INPUTS=${INPUTS:-/root/inputs}
echo "== $(date -u +%FT%TZ) pod_job start; $(nproc) CPUs; $(nvidia-smi --query-gpu=name,memory.total --format=csv,noheader)"
. scripts/pod_setup.sh
echo "waiting for $INPUTS/READY"
until [ -f "$INPUTS/READY" ]; do sleep 20; done
bash "$INPUTS/job.sh"
echo "== $(date -u +%FT%TZ) pod_job done" | tee target/job/DONE
