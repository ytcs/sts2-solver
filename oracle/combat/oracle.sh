#!/usr/bin/env bash
# Thin wrapper: ./oracle.sh run <scenario.json> [--out trace.jsonl] [--random SEED] [--record scen.json] ...
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec dotnet "$here/bin/Release/net9.0/OracleCombat.dll" "$@"
