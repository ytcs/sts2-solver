#!/usr/bin/env bash
# Usage: [GUI=1] bash dev.sh  (rebuild + install the mod, relaunch the game headless, wait for the bridge)
cd "$(dirname "$0")"
taskkill //IM SlayTheSpire2.exe //F >/dev/null 2>&1 && sleep 2
export DOTNET_ROOT=$HOME/.dotnet PATH=$HOME/.dotnet:$PATH
out=$(dotnet build -c Release 2>&1)
echo "$out" | grep -E " error |Build succeeded" | sed 's#.*AgentBridge.src.##; s# \[C:.*##' | sort -u
echo "$out" | grep -q "Build succeeded" || exit 1
if [ -n "$GUI" ]; then cmd //c start "" "steam://rungameid/2868840"; else "/c/Program Files (x86)/Steam/steam.exe" -applaunch 2868840 --headless & fi
for i in $(seq 1 60); do sleep 2; (cd ../.. && python -m agent.bridge s 2>&1 | head -1 | grep -qv "^ERR") && { echo "bridge up after $((i*2))s"; exit 0; }; done
echo "bridge did not come up"; exit 1
