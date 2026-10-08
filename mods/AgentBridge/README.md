# AgentBridge: drive the real game from an agent

In-process mod for STS2 v0.111.0. Serves the current decision as compact text with numbered legal actions on `127.0.0.1:15555` (`STS2_BRIDGE_PORT`). Each `a` runs one action on the game's main thread, waits for the next decision, returns the new state. Agents go through the harness (`python -m agent`, `README.md`), not the bridge directly.

## Commands (`python -m agent.bridge <cmd>`)
| cmd | what |
|---|---|
| `s` / `peek` | state + numbered options (after settling / immediately) |
| `a <i> [e<k>]` | take option i; `e<k>` targets enemy k. Menu new run: `a <i> <character> [ascension] [seed]` |
| `a <i> [<j> ...]` / `a -` | answer a `SELECT` prompt (pick indices / none) |
| `a dp <slot>` | discard a potion |
| `do <json>` | one action in oracle script vocabulary (`{"play":{...}}`, `{"end_turn":true}`) |
| `fight` | JSON `{scenario, log, state}` of the combat (visible information only) |
| `snap`, `deck.json` | JSON snapshots |
| `d` / `p draw\|discard\|exhaust` / `m` | deck+relics+potions / a pile as unordered multiset / act map (`<type>c<col>><children>`, `*` visited) |
| `draw r1c6 ...` / `draw clear` | draw a route on the map |
| `mods` | Custom Run modifiers |
| `f [normal\|fast\|instant]` | fast mode |
| `x <console cmd>` | dev console (tests only; never in a scored run) |
| `t` | debug: every visible clickable control |

Screens (first word): `MENU EVENT MAP COMBAT SELECT REWARDS CARD_REWARD CHOOSE_CARD CHOOSE_RELIC CHOOSE_BUNDLE SHOP RESTSITE TREASURE GAME_OVER MODAL ...`; other overlays fall back to their visible buttons. `(busy)` = still animating: ask again.

## How it works
- `CardSelectCmd.PushSelector` (the game's test hook) answers card prompts via `AgentSelector`; Harmony `PromptPatch` records prompt + source.
- Everything else uses the AutoSlay bot's calls (`ForceClick`, card holder `Pressed`, `CardModel.TryManualPlay`, `PotionModel.EnqueueManualUse`, `EndPlayerTurnAction`, `MerchantEntry.OnTryPurchaseWrapper`). Screen cases: `src/Decisions.cs`.
- Settling: wait for a screen change, then a ready combat turn, a pending choice, or options unchanged for 15 frames. Tutorials off. Mods use a separate `modded/` save profile.

## Build
.NET 9 SDK. `dotnet build -c Release` copies `AgentBridge.dll` + `AgentBridge.json` into `<game>/mods/AgentBridge/`. `./dev.sh`: close the game, rebuild, relaunch through Steam, wait for the bridge.
