---
name: expert-reenact
description: Expert re-enactment: YouTube run video -> run record -> real-game replay on the seed -> per-decision verdicts vs the live player and price. Use when transcribing an expert video, replaying a record, or adjudicating an expert-vs-solver divergence.
---

# Expert re-enactment
Offline `tools/expert.py`; live `python -m agent seedcheck|replay` (`agent/reenact.py`). Plan S8; method E34.

## Steps
1. `tools/expert.py fetch <url> [--creator X] [--fps 1]` -> `data/expert/<creator>/frames/<id>/`, `transcripts/` (git-ignored). Subtitles 429: `--transcript-only` with youtube-transcript-api.
2. Read build stamp + seed from frames. Replay only on build `v0.111.0` (`MODDED` fine); else frame-built fights and notes only.
3. Transcribe every decision in order into `data/expert/<creator>/<id>.json`. Unknown -> `gap` step; never guess.
4. `build <record>` -> `fights/*.json` + sim-vs-frame diffs (each a transcription or fidelity question); `validate <record>` exits 0.
5. Live, main menu, no run in progress, harness only: `python -m agent seedcheck <record> [--custom]` (Neow, map, first fight consistent; stops), then `replay <record> [--steps N] [--until F]` (stops at a gap, a missing recorded action, a game rejection = transcription error, or a skill refusal). Skill gate applies, decision guards do not. Preview offline: `tools/expert.py seedcheck|replay <record> --dry-run`.
   - Pipeline: transcription and live replay run in parallel; the replay follows the record (`--until <first untranscribed floor>`) as floors land, never waits for the whole video. Transcribe in game order, floor blocks complete (fight + rewards + map click) before moving on.
   - Gap or rejection: recover by lookahead, not a guess. The missing action must produce the next recorded state (frames: hand, piles, HP, enemy HP/intent, next screen). Enumerate candidates on the synced simulator (`enumerate_turn`, map/reward options), keep those matching the next observation, and apply the unique match in the game; several matches -> re-read frames; none -> a fidelity question. Record the fix in the record with `note: inferred by lookahead`. `[hyp]` test: share of gaps resolved uniquely per video.
6. `compare <record> [--replay] [--macro] [--fights IDS]` -> `<id>.divergences.json`; `report <record>` -> tables. Notes `data/expert/<creator>/<date>_<id>.md` (template `data/expert/baalorlord/20261008_hMrQSndDvPc.md`).

## Run record
- Top: `video{}`, `seed`, `build`, `modded`, `character`, `ascension`, `boss`, `aliases`, `steps`.
- Step: `floor` (Neow F1), `screen`, `t`, optional `seen`, `room`, `note`, plus one of `pick` (label; REWARDS list; CARD_REWARD name or null; MAP `{room?,col?,row?}` or `"r<row>c<col>"`; SELECT names), `fight` `{id, encounter, enemy_ids, scenario, turns}`, `gap`.
- Turn: `obs` (hp, block, energy, hand, piles, relics, pp, potions, e[{hp, max_hp, block, powers, intent}]), `acts`: `["p", tok, target?]`, `["c", tok...]`, `["pot", slot, target?]`, `["e"]`. Token = alias + `+` upgraded, `*` enchanted, `@k` hand index; target = enemy index in combat order.

## Verdicts
- Live player: `Engine` (`models/current.json`) `decide(seed 1, rounds 8, keep_potions)`, harness objective; his potion use = held.
- Precedence: exact turn enumeration (terminal leaves) > paired full-fight playouts R3 > turn check R2. K=256 search R1 never decides; R2 vs opposite significant R1 = unresolved until R3.
- Gap or expert error: |his - live| > 2 se and >= 1 HP-eq (1 HP = 0.5/max HP). Tie: equal, order only, or |d| + 2 se < 1 HP-eq. Else unresolved.
- Macro: `price` disagreeing by > 2 paired se questions both, decides nothing.
- Enumerator: `enumerate_turn` in `tools/expert.py` (the live Engine's exact turn search is separate).
- A verdict states reference, n, se. A solver gap becomes a bench state + E# line, never a strategy rule.

## Resources
`STS2_DEVICE=cpu` unless the GPU is free; < 6 GB resident, abort below 8 GB free RAM. Never `maturin develop` into a venv another job uses.
