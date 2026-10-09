---
name: expert-reenact
description: Expert re-enactment: YouTube run video -> run record -> real-game replay on the seed -> per-decision verdicts vs the live player and price. Use when transcribing an expert video, replaying a record, or adjudicating an expert-vs-solver divergence.
---

# Expert re-enactment
Offline `tools/expert.py`; live `python -m agent seedcheck|replay` (`agent/reenact.py`). Method E34.

## Steps
1. `tools/expert.py fetch <url> [--creator X] [--fps 1]` -> `data/expert/<creator>/frames/<id>/`, `transcripts/` (git-ignored). Subtitles 429: `--transcript-only` with youtube-transcript-api.
2. Read build stamp + seed from frames. Replay only on build `v0.111.0` (`MODDED` fine); else frame-built fights and notes only.
3. Transcribe every decision in order into a working record `data/expert/<creator>/<id>.json` (frame observations per turn). Unknown -> `gap` step; never guess.
4. `build <record>` -> `fights/*.json` + sim-vs-frame diffs (each a transcription or fidelity question); `validate <record>` exits 0.
5. Live, main menu, no run in progress, harness only: `python -m agent seedcheck <record> [--custom]` (Neow, map, first fight consistent; stops), then `replay <record> [--steps N] [--until F]` (stops at a gap, a missing recorded action, a game rejection = transcription error, or a skill refusal). Skill gate applies, decision guards do not. Preview offline: `tools/expert.py seedcheck|replay <record> --dry-run`.
   - Pipeline: transcription and live replay run in parallel; the replay follows the record (`--until <first untranscribed floor>`) as floors land, never waits for the whole video. Transcribe in game order, floor blocks complete (fight + rewards + map click) before moving on.
   - Gap or rejection: recover by lookahead, not a guess. The missing action must produce the next recorded state (frames: hand, piles, HP, enemy HP/intent, next screen). Enumerate candidates on the synced simulator (`enumerate_turn`, map/reward options), keep those matching the next observation, and apply the unique match in the game; several matches -> re-read frames; none -> a fidelity question. Record the fix in the record with `note: inferred by lookahead`. `[hyp]` test: share of gaps resolved uniquely per video.
6. After the live replay runs end to end: `compact <record> --result ...` -> `<id>.compact.jsonl`, the committed run (actions only); delete the working record and `fights/`. `build <compact>` takes fight states from the game (`replay/<id>/*.json`, else the replay log `replay/<id>.jsonl`; git-ignored); the simulator cannot redraw his hands, so a fresh clone runs `replay <compact>` first.
7. `compare <compact> [--macro --replay] [--fights IDS]` -> `<id>.divergences.json` (rows keyed by `k`, the compact action index); `report` -> tables. Notes `data/expert/<creator>/<date>_<id>.md`.

## Run record
- Compact (committed): header line `video{}`, `seed`, `build`, `modded`, `character`, `ascension`, `boss`, `aliases`, `result`; then one step per line.
- Step: `floor` (Neow F1), `act` (0-2), `screen`, `t`, optional `room`, plus one of `pick` (label; REWARDS list; CARD_REWARD name or null; MAP `{room?,col?,row?}` or `"r<row>c<col>"`; SELECT names; `{discard_potion: slot}`), `fight` `{id, encounter, turns}`, `gap` (working record only).
- Turn: `acts`: `["p", tok, target?]`, `["c", tok...]`, `["pot", slot, target, potion_id]`, `["e"]`; optional `times`, `inferred`. Token = alias + `+` upgraded, `*` enchanted, `@k` hand index; target = the game's enemy index.
- Working record adds per turn `obs` (hp, block, energy, hand, piles, relics, pp, potions, e[{hp, max_hp, block, powers, intent}]) and per fight `enemy_ids`, `scenario`.

## Verdicts
- Live player: `Engine` (`models/current.json`) `decide(seed 1, rounds 8, keep_potions)`, harness objective; his potion use = held.
- Precedence: exact turn enumeration (terminal leaves) > paired full-fight playouts R3 > turn check R2. K=256 search R1 never decides; R2 vs opposite significant R1 = unresolved until R3.
- Gap or expert error: |his - live| > 2 se and >= 1 HP-eq (1 HP = 0.5/max HP). Tie: equal, order only, or |d| + 2 se < 1 HP-eq. Else unresolved.
- Macro: `price` disagreeing by > 2 paired se questions both, decides nothing.
- Enumerator: `enumerate_turn` in `tools/expert.py` (the live Engine's exact turn search is separate).
- A verdict states reference, n, se. A solver gap becomes a bench state + E# line, never a strategy rule.

## Macro corpus (no combat, no replay; `tools/expert_macro.py` via `tools/expert.py macro*`)
1. `macro <url> --creator X`: frames (1 fps) + scan -> `work/<id>/index.txt` (non-combat segments, kNN on `data/expert/work/screen_exemplars.npz`; rebuild: `macro-scan hMrQSndDvPc --creator baalorlord --train <its compact>`) and `work/<id>/overview/NN.jpg` (16 segments per grid). Labels are noisy: read the frames, not the label.
2. Read: overview grids for the flow; `macro-crop <id> --creator X --at m:ss,... [--region cards|center|map|top|full|x0,y0,x1,y1] [--top] [--scale .5]` for text. Top bar = HP, gold, potion belt, floor, deck count; the clock in it is in-game time, crop labels are video time. A hover tooltip can hide a card name: read another frame.
3. Transcribe in game order into `data/expert/<creator>/work/<id>.macro.work.json` (git-ignored): header `video`, `build`, `modded`, `seed`, `character`, `ascension`, `result`, `base {max_energy, max_potion_slots, base_orb_slots}`, `start {obs, deck ["Strike x5", ...], relics}`, `acts {"0": {name, bosses, map?}}`, `steps`.
   - Step keys (each with `f` floor, `t` video time at the decision): decisions `ancient`/`event` (+`opts` option heads, `pick`, `then`, `fx`), `card` [3 names, `+` upgraded] + `pick` (null = skip), `rest` smith|rest (+`then` [target]), `shop` {cards/relics/potions [[name, price]], remove price} + `buy` [names | "remove"] + `then` [removed card], `potion` offered + `drop` held name | null (left it), `map` "r<row>c<col>" + `opts` ["M r2c1", ...] (types M E R $ T ?; forced moves may be omitted). Non-decisions: `fight` M|E|B|ENCOUNTER_ID, `rewards` {gold, potion (dropped), relic}, `gain`/`fx` {cards+, cards-, up, ench [[card, id]], transform [[a, b]], relics+, relics-, potions+, potions-, gold, hp, max_hp, slots}, `obs`, `deckview` [names].
   - `obs` "HP/MAX Ggold Ddeck Rrelics P[a,b]" on every decision (read at that screen, before the choice). Unknown -> leave it out and add `note`; never guess.
   - Act map (optional, best value for map decisions): `acts[n].map` in `routes.parse_map` text (`r1: Mc1>0,2 ?c3>4`, columns 0-6); a partial map is padded with template rooms.
4. `macro-build <work>`: tracks deck/relics/gold/potions; every `obs`/`deckview` mismatch is an error (fix the work file; `--allow` records them). Writes `<id>.macro.jsonl` (committed).
5. `macro-compare <record> [--n 128]` -> `<id>.macro_verdicts.json`; `macro-replay <log> --compact <c>` makes the record from a live replay log (game truth).

## Resources
`STS2_DEVICE=cpu` unless the GPU is free; < 6 GB resident, abort below 8 GB free RAM. Never `maturin develop` into a venv another job uses.
