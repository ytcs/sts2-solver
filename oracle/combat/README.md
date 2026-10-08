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
| `check-shuffle S.json --trace T.jsonl` | opening hand + draw == `UnstableShuffle(deck)` |

Options: `--max-steps` (default 400), `--max-rounds` (60), `--lenient` (do not abort on game `Log.Error`), `--verbose`. Exit 1 on oracle error (stderr).

## Files
`Program.cs` CLI, `Boot.cs` init, `GodotStub.cs` + `Patches.cs` stubs, `Pump.cs` single-thread scheduler, `Scenario.cs` input, `Setup.cs` scenario -> run/combat, `Driver.cs` stepper + selector + legal actions, `Dump.cs` trace schema, `Fuzz.cs`, `Catalog.cs`, `Pools.cs`. Recorded regression traces: `oracle/regression/` (replayed by `cargo test -p sts2diff --test regression`).
