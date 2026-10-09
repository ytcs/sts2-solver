---
name: expert-reenact
description: Expert re-enactment: YouTube run video -> run record -> real-game replay on the seed -> per-decision verdicts vs the live player and price. Use when transcribing an expert video, replaying a record, or adjudicating an expert-vs-solver divergence.
---

# Expert re-enactment
Offline `tools/expert.py`; live `python -m agent seedcheck|replay` (`agent/reenact.py`). Plan item 7; method E34.

## Steps
1. `python tools/expert.py fetch <url> [--creator X] [--fps 1]` -> `data/expert/<creator>/frames/<id>/<mmss>.jpg`, `transcripts/<date>_<id>.txt` (git-ignored; video deleted). Subtitles HTTP 429 -> youtube-transcript-api (this interpreter or `py -3.13`); `--transcript-only` retries that alone.
2. Read the build stamp and the seed from frames. Build gate: replay only if the build starts `v0.111.0`; `MODDED (n)` is fine (creators' mods: QoL/streaming, RNG untouched). Other builds: frame-built fights and notes only.
3. Transcribe every decision, in order, into `data/expert/<creator>/<id>.json` (format below) by reading frames. Unknown -> `gap` step. Never guess.
4. `tools/expert.py build <record>` -> `fights/<fight id>.json` (simulator synced to every observation; the printed report = sim-vs-frame diffs, each a transcription or fidelity question). `validate <record>` exits 0.
5. Live, game up, at the main menu with no run in progress, through the harness only:
   - `python -m agent seedcheck <record>`: starts the seeded standard run (`--custom`: custom run), checks Neow `seen`, map boss + paths consistent with the record's rooms by floor, the first fight's opening; stops. An ambiguous map node is guessed and logged; `replay` then refuses that run: abandon it by hand, rerun from the menu.
   - `python -m agent replay <record> [--steps N] [--until F]`: resumes from its log; `--until F` stops before the map pick into floor F. Stops at a gap, a screen not offering the recorded action (`seen` mismatch: seeding path (standard vs custom), profile unlocks, or transcription), a game rejection (transcription error), a skill refusal (load the skill, rerun).
   - Screens demand `sts2-pathing`, `sts2-deckbuilding`, `sts2-mechanics` besides the core three. Decision guards do not apply (the expert decided); the skill gate does.
   - Logs: `replay/<id>.jsonl` (true screen, deck, fight state per decision), `replay/<id>/<fight>.json`, `runs/<run>/events.jsonl`.
   - Offline preview: `tools/expert.py seedcheck|replay <record> --dry-run [--all]`.
6. `tools/expert.py compare <record> [--replay] [--macro]` -> `<id>.divergences.json` (`--fights NONE --macro` adds macro rows to an existing output); `report <record> [--out f.md]` -> tables. Notes `data/expert/<creator>/<date>_<id>.md`, template `data/expert/baalorlord/20261008_hMrQSndDvPc.md`: header (url, build, seed, sources), method, macro table, per fight: state, his line, his words, verdict table; solver gaps as bench states; caveats.

## Run record
- Top: `video{id,url,creator,channel,title,uploaded}`, `seed`, `build`, `build_hash`, `modded`, `character`, `ascension`, `boss`, `aliases` (card tokens), `steps`.
- Step: `floor` (game counter: Neow F1, first room F2), `screen`, `t` (video time), optional `seen` (offered labels, verified live), `room`, `note`; plus one of:
  - `pick`: label prefix/substring (EVENT, RESTSITE, SHOP, TREASURE); REWARDS list of labels, one click each (`proceed` skips the rest); CARD_REWARD card name (`+` upgraded) or null = skip; MAP `{room?, col?, row?}` or `"r<row>c<col>"` (`{}`: resolved by the record's rooms when unique); SELECT list of card names.
  - `fight`: `{id, encounter, video_span, enemy_ids, scenario, turns, end_obs?, hand_enchanted?}`; `scenario` = deck (enchantments), relics + counters, potions with slots, HP before combat.
  - `gap`: why the action is unknown.
- Rooms by floor: `room`, else the encounter suffix; set `room` on Neow (`Ancient`) and events (`Event`).
- Turn: `obs` (state at turn start), `times` (one per act), `acts`.
  - obs keys: hp, block, energy, hand (tokens left to right), draw/discard/exhaust (when the pile is shown), draw_n, discard_n, relics {id: counter}, pp {power: amount}, potions, e [{hp, max_hp, block, powers, intent [[type, dmg, hits]]}] (alive enemies, screen order).
  - acts: `["p", tok, target?, obs?]`, `["c", tok...]` (selection picks), `["pot", slot, target?]`, `["e"]` (last act of the turn). A trailing dict = what the screen showed after that act.
  - token = alias + `+` upgraded, `*` enchanted copy, `@k` hand index; target = enemy index in combat order (dead keep theirs, summons append).

## References and verdicts (rule)
- Live player: `Engine` (models/current.json) `decide(seed 1, rounds 8, keep_potions)`, harness fight objective; a reproducible proxy for the harness's danger-scaled budget. His potion use = `held` (the operator commits potions).
- Precedence: exact turn enumeration (every line to end of turn, all leaves terminal) > paired full-fight playouts R3 > turn check R2 (same enumeration, search leaves). The K=256 search R1 never decides; R2 against a significantly opposite R1 = unresolved until R3.
- Verdict (our gap / expert error): |his - live| > 2 se and >= 1 HP-eq (q: win 1 + 0.5 HP/max HP, loss -1; 1 HP = 0.5/max HP). Tie: exact equality, order only, or |d| + 2 se < 1 HP-eq. Else unresolved.
- Macro: `price` best vs his pick on the ladder's horizon; "price prefers another option" (> 2 paired se) questions both, decides nothing.
- `enumerate_turn` (`tools/expert.py`) is the swappable enumerator (item 8 replaces it).

## Resources
- `STS2_DEVICE=cpu` unless the GPU is free; one heavy process; < 6 GB resident; abort below 8 GB free RAM (`compare` runs a watchdog; the enumerator caps 4000 lines, keeps hashes only, checks RAM every 50 lines).
- Never `maturin develop` or pip-install into a venv another job uses.

## Claims
- Expert claims are validation cases, never rules to encode; evidence decides.
- A verdict states its reference, n and se; solver results are lower bounds under the stated solver.
- A solver gap becomes a bench state (fight record + log index) and an E# line, not a strategy rule.
