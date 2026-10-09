# OracleCombat: real STS2 combat from the game's own code, full state traces

Runs the real `sts2.dll` combat logic in a plain `dotnet` process (no Godot runtime, Steam or mod loader; nothing in the game dir is modified). Stubs, scenario schema, trace, gotchas: `docs/simulator.md` "Oracle".

## Build
```
cd oracle/combat && dotnet build -c Release    # .NET 9 SDK, x86-64; -p:GameDir=<data_sts2_* dir> to override
```
`GameDir` defaults to the Steam `data_sts2_windows_x86_64` (Windows) or `~/.local/share/Steam/.../data_sts2_linuxbsd_x86_64` (Linux). `oracle.sh` needs bash; on Windows call `dotnet bin/Release/net9.0/OracleCombat.dll <cmd>` (what `tools/fuzz_gen_mix.py` does). A `SentryGodotInitializer` line goes to stdout at start-up: always use `--out`.

## Commands (`./oracle.sh <cmd>`)
| cmd | what |
|---|---|
| `run S.json --out T.jsonl` | scripted fight (`script` drives it) |
| `run S.json --random SEED [--policy random\|playall\|stall] --out T.jsonl --record R.json` | random legal-action driver; `--record` writes a scenario + exact script that replays byte-identically |
| `batch DIR [--max-steps N --max-rounds N]` | every `DIR/*.scenario.json` in one process (~10 ms/fight): `policy` key or the scenario's `script`; writes `NAME.jsonl` + `NAME.res` or `NAME.err` (used by `tools/fuzz_gen_mix.py`) |
| `fuzz --character C --encounters ALL --seeds 1-20 --out-dir D` | random scenarios in one process; failures leave scenario + trace |
| `catalog --out F` | pools and encounters (input of `tools/gen_train.py`, `tools/fuzz_gen_mix.py`; copy in `data/catalog.json`) |
| `dump-pools --out F` / `list-meta --out F` | pools, encounters (act, room), enchantment applicability |
| `dump-rng SEED --out F` | the nine fresh streams for a seed string |
| `run-replay R.compact.jsonl --out T.jsonl` | whole run from the record's seed through the game's run code (Neow, map, combats, rewards, treasure, rest, shop, events, acts); ~2 s; check vs a live replay log: `tools/run_replay_verify.py T.jsonl LIVE.jsonl` |
| `check-shuffle S.json --trace T.jsonl` | opening hand + draw == `UnstableShuffle(deck)` |
| `serve --port N [--state-dir D] [--seed S --character C] [--ascension 10] [--pck P]` | headless game server speaking the bridge mod's protocol (below); starts at the menu, or straight into a seeded run |

Options: `--max-steps` (default 400), `--max-rounds` (60), `--lenient` (do not abort on game `Log.Error`), `--verbose`. Exit 1 on oracle error (stderr).

## Headless server (`serve`)
One game per process, ~150 MB, CPU only; the harness reaches it with `STS2_BRIDGE=127.0.0.1:N` (`agent/bridge.py`; with it set the bridge never falls back to the real game, and `python -m agent` defaults its daemon port to N+1). Never use port 15555.
- Protocol = `mods/AgentBridge` (`Server.cs` line in / text out, `Commands.cs`): `s peek a do fight snap deck.json d p m mods f`; `x` (dev console) and `draw` are unsupported, `t` prints the screen kind. Extra: `menu` (save-and-quit to the menu, in memory: the menu then offers continue / abandon with the confirm modal), `shutdown`.
- Shared source, not a port: the mod's `Text.cs` (all labels), `Snap.cs` (`fight`/`snap`/`deck.json` JSON, fight log), `AgentSelector.cs` + `PromptPatch.cs` (SELECT prompts) are compiled in (`OracleCombat.csproj`). `Ui.cs` ports `Decisions.cs`/`Commands.cs` onto the model: screens and labels identical, Godot node reads replaced by the state the nodes hold (map open, reward screens, chest opened, rest choice made) and the UI code's calls (`NRewardsScreen` proceed incl. boss -> `ActChangeSynchronizer`, `NMapScreen.RecalculateTravelability` + `VoteForMapCoordAction`, `NEventRoom` Proceed, `NTreasureRoom`, `NRestSiteRoom`, `NMerchantCardRemoval`, `NCombatUi.OnCombatWon` -> `OfferRoomEndRewards`).
- Prompts the game only shows through nodes: card reward (synchronous selector call: the server answers the command in flight with the CARD_REWARD screen and keeps serving until it is answered), `RelicSelectCmd.FromChooseARelicScreen` (CHOOSE_RELIC), `FromChooseABundleScreen` (CHOOSE_BUNDLE, TestMode took `bundles[0]`), `NCrystalSphereScreen` (CRYSTAL_SPHERE).
- Real English text: `Loc.cs` reads `localization/eng/*.json` from `SlayTheSpire2.pck` (read-only) into `LocManager`; formatting errors return the raw text as in the shipped game.
- No save I/O: every `GodotFileIo` call is confined to `--state-dir` (dropped without one; `SaveIsolation.cs`); settings/prefs are the in-memory test saves; profile = `UnlockState.all`, no progress file.
- Checks: `tools/serve_replay.py REC --port N --start --live LIVE` (protocol-level re-enactment, ~6 s), `tools/serve_replay_check.py` (the harness's `replay`), `tools/serve_harness_run.py` (harness plays a seeded run: `combat` + macros), `tools/serve_bench.py --n 8` (parallel random-policy runs, RSS, save-folder check).
- Known differences from the real game: fixed fully unlocked profile (`UnlockState.all`; ActModel discovery override and first-run tutorial rooms off; profile counters, e.g. Test Subject's name `#C{kills+8}` shows `#C8`); GAME_OVER is one page `Floors Climbed: N` / `main menu` (no "Epoch Unlocked!" page, no run history); ancient dialogue (`continue dialogue`) never shown; the fake-merchant event's shop is not modelled; FTUEs off, NonInteractive (no animations/waits); `x` console unsupported. Cauldron / Calling Bell use the shipped random rewards (`Patches.RealRunRng`, also in `run-replay`).

## Files
`Program.cs` CLI, `Serve.cs` server loop + prompts, `Ui.cs` screens, `Loc.cs` localization, `SaveIsolation.cs`, `BridgeShim.cs`, `RunReplay.cs` whole-run replay, `Boot.cs` init, `GodotStub.cs` + `Patches.cs` stubs, `Pump.cs` single-thread scheduler, `Scenario.cs` input, `Setup.cs` scenario -> run/combat, `Driver.cs` stepper + selector + legal actions, `Dump.cs` trace schema, `Fuzz.cs`, `Catalog.cs`, `Pools.cs`. Recorded regression traces: `oracle/regression/` (replayed by `cargo test -p sts2diff --test regression`).
