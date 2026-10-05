---
name: sts2-core
description: Operating rules for playing Slay the Spire 2 with the self-play harness (python -m agent): objective, no-cheating rules, commands, how to use the solver and eval, safety rules, and the review loop. Load at the start of any STS2 run.
---

# STS2 core

## Objective and honesty
Win the run; secondary: most HP left. HP is a resource, but never spend it below what the route needs. Allowed information: the screen, public game knowledge (pools, card text, monster patterns), and what I observed this run. Not allowed in a scored run: dev console or god mode, hidden state (draw order, RNG, the pre-rolled encounter and elite order), restarts or reloads, the run seed's future. Dev-console use is for harness tests only.

## Commands (`python -m agent <cmd>`; details in `agent/harness.py`)
`s` state, `a <i> [target] [-- why]` act (always give the reason), `d` deck/relics, `p draw|discard|exhaust`, `m` map, `draw r1c6 r2c6 ...` draw the planned route on the map, `relics` (counters such as Pen Nib).
`adv [secs]` solver advice plus the enemies' expected damage over the next turns; `turn [secs]` / `combat [secs]` let the solver play. `budget <s>` sets the default search time (1 s).
`eval` combat value of deck variants against encounter pools; `route M E R ... --hp N` HP budget along a route; `note`, `status`, `newrun`.

## Combat
- Ask `adv` at every non-trivial decision and play its line; deviate only for something the solver cannot weigh (a potion kept for the boss, a route consequence) and say why. My manual lines against Vantom cost about 25 HP more than the solver's predicted result. `[played]`
- Budget: 0.3-1 s for obvious turns, 5-20 s when HP is low, a boss or elite is on the line, or the top options are close. If `adv` says `SIMULATOR DESYNC` or `DIFFERS`, the advice is void: play by hand and run `status`.
- Check `sts2-mechanics` and the power descriptions before big hits: Slippery, Hard to Kill and Burrowed change what a good turn is. Read relic counters (`relics`) and aim the 10th Pen Nib attack at the best target.
- Never use the dev console, never reload mid-fight.

## Macro
- `eval` prices what a card, relic, removal or upgrade does in fights against the pools ahead (weak / regular / elite / boss of the act, see `sts2-acts`), at the HP I will arrive with. Include the boss pool: it is the usual weakness. It does not price the deck slot, rewards still to come, gold, rest actions, Clone-like effects, or route interplay: price those by hand.
- Route: read the whole map (visible at the ancient), plan with `route`, and commit one node at a time. Both rests before a boss are heals if HP binds; smith only with spare HP. Read every map option before clicking; a click onto an elite or boss below 60% HP is refused unless confirmed with `!`. Never chain map or node choices.
- Relics, ancients and boss relics interact with the route (rests, shops, elites): compare whole plans, not single fights.
- Card picks are a hypothesis space, not rules: threshold-and-skip (take only gains that clear ~2 SE and matter on the pools ahead), cover the known weakness, and speculative scaling picks. Rares deserve a longer-term evaluation than the next pool; a marginal common is usually a skip.
- Potions: keep the strongest one for the act's boss unless the solver sees a clear loss without it.
- Shop heals (Meal Ticket) and act-transition heals (HP resets to about 80% on arrival) belong in the route budget.

## Review loop (after each run)
`python -m agent.improve review` (predicted vs actual, search overrides, fidelity), then fix fidelity first, test the `[hyp]` claims the run touched (`python -m agent.improve lessons` lists them), update the skills with the evidence tag and numbers, and only then consider `corpus` / `finetune` / `gate` for the model. Keep the book lean: one line per lesson, replace rather than append.

## Measured `[sim]` / `[played]`
- A10 Ironclad starter deck beats Act 1 weak and regular fights, loses to the first elite.
- Shakedown run: Act 1 cleared (boss at 13/80 HP), died at Act 2 floor 24 after entering an elite at 37/80 by a chained map click. Deck evals at the end of Act 1: Act 2 boss win about 30-50%, elites 94-99% at ~50% HP cost.
