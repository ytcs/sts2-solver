#!/usr/bin/env bash
# Headless game for an operator run: bash scripts/operator_server.sh SEED [PORT] [CHARACTER]; then STS2_BRIDGE=127.0.0.1:PORT python -m agent <cmd>
set -euo pipefail
cd "$(dirname "$0")/.."
SEED=$1; PORT=${2:-15700}; CHAR=${3:-ironclad}
if (exec 3<>/dev/tcp/127.0.0.1/$PORT) 2>/dev/null; then echo "port $PORT is in use (another server?): stop it first"; exit 1; fi
D=target/operator/$SEED/server
mkdir -p $D/appdata $D/localappdata $D/tmp $D/saves
DOTNET=${DOTNET:-$HOME/.dotnet/dotnet.exe}
APPDATA=$PWD/$D/appdata LOCALAPPDATA=$PWD/$D/localappdata TEMP=$PWD/$D/tmp TMP=$PWD/$D/tmp \
  nohup "$DOTNET" "$PWD/oracle/combat/bin/Release/net9.0/OracleCombat.dll" serve --port $PORT --state-dir "$PWD/$D/saves" \
  --seed $SEED --character $CHAR --ascension 10 >$D/server.log 2>&1 &
echo "server pid $! port $PORT seed $SEED"
