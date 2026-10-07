# OracleCombat: real STS2 combat from the game's own code, with full state traces

Runs the real `sts2.dll` combat logic in a plain `dotnet` process (no Godot runtime, no Steam, no mod loader).
Design notes, stubbing details, trace schema and gotchas: `docs/oracle.md`.

## Build
```
cd oracle/combat
dotnet build -c Release            # needs the game install; override with -p:GameDir=/path/to/data_sts2_linuxbsd_x86_64
```
Default `GameDir` = `~/.local/share/Steam/steamapps/common/Slay the Spire 2/data_sts2_linuxbsd_x86_64` (same as `oracle/RngGolden`).
Requires .NET 9 SDK, x86-64 (the Godot stub is raw x86-64 machine code, allocated with `mmap` on Linux and `VirtualAlloc` on Windows). On Windows `GameDir` defaults to the Steam `data_sts2_windows_x86_64` folder; `oracle.sh` needs bash, so `tools/fuzz_gen_mix.py` calls `dotnet bin/Release/net9.0/OracleCombat.dll` directly there (`dotnet` on `PATH`). Nothing in the game directory is modified. A harmless
`SentryGodotInitializer: ...` line is printed to stdout at start-up by the game assembly; always use `--out`.

## Run
```
./oracle.sh run SCENARIO.json --out trace.jsonl                       # scripted: scenario["script"] drives the fight
./oracle.sh run SCENARIO.json --random SEED --out trace.jsonl --record replayable.scenario.json
        # random legal-action driver (seeded System.Random). --record writes the scenario + the exact chosen script
        # (including `choose` answers), which replays to a byte-identical trace.
./oracle.sh fuzz --character IRONCLAD --encounters ALL --seeds 1-20 --out-dir /tmp/fz [--keep-all] [--ascension N]
        # many random scenarios (starter + random cards/relics/potions, random hp/floor) in ONE process (~50 ms each);
        # failing runs leave NAME.scenario.json (+ error) and NAME.jsonl in --out-dir. python3 fuzzsum.py DIR groups errors.
./oracle.sh batch DIR [--max-steps N --max-rounds N]
        # run every DIR/*.scenario.json in ONE process (~10 ms/fight); scenario key "policy": {"kind":"random|playall|stall","seed":N,
        # "max_steps":N,"max_rounds":N}; writes NAME.jsonl + NAME.res ("result nactions") or NAME.err. Without "policy" the scenario's
        # own "script" is replayed (regression scenarios, oracle/regression/). Driven by tools/fuzz_gen*.py (randomised differential fuzzing).
./oracle.sh catalog --out catalog.json     # pools and encounters for the generators (`tools/gen_train.py`, `tools/fuzz_gen_mix.py`)
./oracle.sh list-meta --out meta.json      # pools / encounters metadata for `tools/fuzz_gen_orb_pet.py`
./oracle.sh dump-pools --out pools.json    # card / relic / potion pools, encounters (act, room type), enchantment applicability
./oracle.sh dump-rng SEED_STRING --out rng.json      # fresh nine streams {counter,s0..s3} for RunRngSet(seed)
./oracle.sh check-shuffle SCENARIO.json --trace trace.jsonl   # opening hand+draw == UnstableShuffle(deck, Rng(hash(seed),"shuffle"))
```
`run` also takes `--policy random|playall|stall` (with `--random SEED`): `random` = uniform over legal actions, `playall` = end the turn
only when nothing else is legal, `stall` = never plays an Attack (reaches deep turns).
Options: `--max-steps N` (random driver, default 400), `--max-rounds N` (default 60), `--lenient` (do not abort on game `Log.Error`),
`--verbose` (print game Info/Debug logs to stderr). Exit code 1 on oracle error (message on stderr).

## Files
`Program.cs` CLI, `Boot.cs` init sequence, `GodotStub.cs` + `Patches.cs` Godot/engine stubbing, `Pump.cs` single-thread
scheduler, `Scenario.cs` input schema, `Setup.cs` scenario -> Player/RunState/combat, `Driver.cs` stepper + choice selector
+ legal-action enumeration, `Dump.cs` trace schema, `Fuzz.cs` random scenario generator.
Samples: `oracle/samples/`, example scenarios: `oracle/scenarios/`.
