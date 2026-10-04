#!/usr/bin/env bash
# Restores the git-ignored working directories from the tracked copies (run once on a fresh checkout):
#   target/train/{catalog,train,eval,mid,iron0_*}.json   scenario sets and the card / relic / encounter catalog the generators read
#   target/runs/{phrog,phrog_dup}.json                    example fights
#   target/exit/mine1.npz                                 first disagreement-mining archive
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p target/train target/runs target/exit
cp data/catalog.json target/train/catalog.json
cp data/train/*.json target/train/
cp data/examples/*.json target/runs/
cp data/analysis/*.json target/runs/ 2>/dev/null || true
cp data/analysis/*.npz target/exit/ 2>/dev/null || true
echo "restored: $(ls target/train | wc -l) scenario files in target/train"
