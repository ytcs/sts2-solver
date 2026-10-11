---
name: expert-reenact
description: Expert re-enactment: YouTube run video -> run record -> real-game replay on the seed -> per-decision verdicts vs the live player and price. Use when transcribing an expert video, replaying a record, or adjudicating an expert-vs-solver divergence.
---

# Expert re-enactment
Offline `tools/expert.py`; live `python -m agent seedcheck|replay` (`agent/reenact.py`). Method E34.

## Steps
1. `tools/expert.py fetch <url> [--creator X] [--fps 1] [--transcript-only]` -> `data/expert/<creator>/frames/<id>/`, `transcripts/` (git-ignored). One download at a time (bursts trigger YouTube's bot check; cookies only with the user's consent).
2. Read build stamp + seed from frames. Replay only on build `v0.111.0` (`MODDED` fine); else frame-built fights and notes only. Run result from the audio of the last minutes (a post-boss screen reads like game over in frames).
3. Transcribe every decision in game order into `data/expert/<creator>/<id>.json` (frame observations per turn), floor blocks complete (fight + rewards + map click). Unknown -> `gap` step; never guess.
4. `build <record>` -> `fights/*.json` + sim-vs-frame diffs (each a transcription or fidelity question); `validate <record>` exits 0.
5. Per floor block, headless first: `dotnet oracle/combat/bin/Release/net9.0/OracleCombat.dll run-replay <compact.jsonl> --out trace.jsonl` (`DOTNET_ROOT=$HOME/.dotnet`; E41) must pass; then `tools/expert.py check-trace <record> trace.jsonl` (frame hand, HP, enemy HP vs trace) catches silent divergences.
6. Live, main menu, no run, harness only: `python -m agent seedcheck <record> [--custom]` (Neow, map, first fight), then `replay <record> [--steps N] [--until F]`; stops at a gap, a missing action, a game rejection (= transcription error) or a skill refusal. Skill gate applies, decision guards do not. Offline preview: `tools/expert.py seedcheck|replay <record> --dry-run`. Replay follows transcription (`--until <first untranscribed floor>`), never waits for the whole video.
7. Gap or rejection: recover by lookahead. Enumerate candidates on the synced simulator (`enumerate_turn`, map/reward options), keep those producing the next observation; unique -> apply, record `note: inferred by lookahead`; several -> re-read frames; none -> fidelity question. `[hyp]` test: share of gaps resolved uniquely per video.
8. After an end-to-end live replay: `compact <record> --result ...` -> `<id>.compact.jsonl` (committed, actions only); delete the working record and `fights/`. `build <compact>` takes fight states from `replay/<id>/*.json` or `replay/<id>.jsonl` (git-ignored): a fresh clone runs `replay <compact>` first.
9. `compare <compact> [--macro --replay] [--fights IDS]` -> `<id>.divergences.json` (rows keyed by `k`, compact action index); `report` -> tables. Notes `data/expert/<creator>/<date>_<id>.md`.

## Run record
- Compact header: `video{}`, `seed`, `build`, `modded`, `character`, `ascension`, `boss`, `aliases`, `result`; then one step per line.
- Step: `floor` (Neow F1), `act` (0-2), `screen`, `t`, optional `room`, plus one of `pick` (label; REWARDS list; CARD_REWARD name or null; MAP `{room?,col?,row?}` or `"r<row>c<col>"`; SELECT names; `{discard_potion: slot}`; `{potion: slot, id}`), `fight` `{id, encounter, turns}`, `gap` (working record only).
- Turn `acts`: `["p", tok, target?]`, `["c", tok...]`, `["pot", slot, target, potion_id]`, `["e"]`; optional `times`, `inferred`. Token = alias + `+` upgraded, `*` enchanted, `@k` hand index; target = game enemy index.
- Working record adds per turn `obs` (hp, block, energy, hand, piles, relics, pp, potions, e[{hp, max_hp, block, powers, intent}]) and per fight `enemy_ids`, `scenario`.

## Verdicts
1. Live player: `Engine` (`models/current.json`) `decide(seed 1, rounds 8, keep_potions)`, harness objective; his potion use = held.
2. Precedence: exact turn enumeration > paired full-fight playouts R3 > turn check R2. K=256 search R1 never decides; R2 vs an opposite significant R1 = unresolved until R3.
3. Gap or expert error: |his - live| > 2 se and >= 1 HP-eq (1 HP = 0.5/max HP). Tie: equal, order only, or |d| + 2 se < 1 HP-eq. Else unresolved.
4. Macro: `price` disagreeing by > 2 paired se questions both, decides nothing.
5. A verdict states reference, n, se. A solver gap becomes a bench state + E# line, never a strategy rule.

## Macro corpus (no combat; `tools/expert.py macro*` -> `tools/expert_macro.py`)
1. `macro <url> --creator X`: frames + scan -> `work/<id>/index.txt` (non-combat segments; kNN on `data/expert/work/screen_exemplars.npz`, rebuild `macro-scan hMrQSndDvPc --creator baalorlord --train <its compact>`) and `work/<id>/overview/NN.jpg`. Labels are noisy: read frames.
2. `macro-crop <id> --creator X --at m:ss,... [--region cards|center|map|top|full|x0,y0,x1,y1] [--top] [--scale .5]`. Top bar = HP, gold, belt, floor, deck count (its clock is in-game time). A hover tooltip can hide a name: read another frame.
3. Transcribe in game order into `data/expert/<creator>/work/<id>.macro.work.json` (git-ignored): header `video`, `build`, `modded`, `seed`, `character`, `ascension`, `result`, `base {max_energy, max_potion_slots, base_orb_slots}`, `start {obs, deck ["Strike x5", ...], relics}`, `acts {"0": {name, bosses, map?}}`, `steps`.
   - Steps (each `f` floor, `t` video time): `ancient`/`event` (`opts`, `pick`, `then`, `fx`), `card` [3 names] + `pick` (null = skip), `rest` smith|rest (+`then`), `shop` {cards/relics/potions [[name, price]], remove price} + `buy` [names | "remove"] + `then`, `potion` + `drop` (held name | null), `map` "r<row>c<col>" + `opts` ["M r2c1", ...] (M E R $ T ?). Non-decisions: `fight` M|E|B|ENCOUNTER_ID, `rewards` {gold, potion, relic}, `gain`/`fx` {cards+, cards-, up, ench, transform, relics+, relics-, potions+, potions-, gold, hp, max_hp, slots}, `obs`, `deckview`.
   - `obs` "HP/MAX Ggold Ddeck Rrelics P[a,b]" on every decision, read before the choice; unknown -> omit + `note`.
   - Act map (optional): `acts[n].map` in `routes.parse_map` text (`r1: Mc1>0,2 ?c3>4`, columns 0-6); partial maps padded with template rooms.
4. `macro-build <work>`: every `obs`/`deckview` mismatch is an error (`--allow` records them) -> `<id>.macro.jsonl` (committed).
5. `macro-compare <record> [--n 128] [--seed S]` -> `<id>.macro_verdicts.json`; claim only verdicts stable across >= 2 seed blocks. `macro-summary <verdicts...>` pools; `macro-replay <log> --compact <c>` builds the record from a live replay log; `macro-diff <frames record> <reference>` scores a transcription.

## Resources
`STS2_DEVICE=cpu` unless the GPU is free; < 6 GB resident, abort below 8 GB free RAM. Never `maturin develop` into a venv another job uses.
