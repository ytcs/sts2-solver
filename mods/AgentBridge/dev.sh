#!/usr/bin/env bash
# Dev loop: close the game, rebuild + install the mod, relaunch through Steam, wait for the bridge.
cd "$(dirname "$0")"
taskkill //IM SlayTheSpire2.exe //F >/dev/null 2>&1 && sleep 2
export DOTNET_ROOT=$HOME/.dotnet PATH=$HOME/.dotnet:$PATH
out=$(dotnet build -c Release 2>&1)
echo "$out" | grep -E " error |Build succeeded" | sed 's#.*AgentBridge.src.##; s# \[C:.*##' | sort -u
echo "$out" | grep -q "Build succeeded" || exit 1
cmd //c start "" "steam://rungameid/2868840"
for i in $(seq 1 60); do sleep 2; (cd ../.. && python -m agent.bridge s) >/dev/null 2>&1 && { echo "bridge up after $((i*2))s"; exit 0; }; done
echo "bridge did not come up"; exit 1
