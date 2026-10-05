# AgentBridge: drive the real game from an agent

An in-process mod for Slay the Spire 2 (v0.111.0) that serves the current decision as compact text, with numbered legal actions, on
`127.0.0.1:15555` (`STS2_BRIDGE_PORT`). Each `a` command runs one action on the game's main thread, waits until the game reaches the next decision,
and returns the new state. A whole turn of play is one short exchange, so the protocol is cheap in tokens.

```
$ python -m agent.bridge s
COMBAT
A1 F2 IRONCLAD A10 HP 64/80 G99 pots[-, -]
T1 E3/3 draw7 disc0 exh0
you b0
e0 Corpse Slug 27/27 b0 -> atk 3x2 [Ravenous 5]
e1 Corpse Slug 28/28 b0 -> atk 9 [Ravenous 5]
play: a <i> [e<target>]
0 Defend(1) Gain 5 Block.
1 Strike(1) Deal 6 damage. ->e
2 Bash(2) Deal 8 damage. Apply 2 Vulnerable. ->e
...
5 end turn
$ python -m agent.bridge a 2 e1        # Bash the second slug
```

## Commands (`python -m agent.bridge <cmd>`)
| cmd | what |
|---|---|
| `s` | current state + numbered options (waits for the game to settle first) |
| `a <i> [args]` | take option i. Combat: `e<k>` targets enemy k (needed for `->e` options when there are several enemies). Main menu "new run": `a <i> <character> [ascension] [seed]` |
| `a <i> [<j> ...]` / `a -` | answer a `SELECT` card-choice prompt (hand discards, exhausts, upgrades, transforms, removals, grids...) |
| `a dp <slot>` | discard a potion (0-based slot), e.g. to take a potion reward with a full belt |
| `d` | deck, relics, potions |
| `p draw\|discard\|exhaust` | a combat pile, as an unordered multiset (the draw order is hidden, as for a player) |
| `m` | the whole act map (`<type>c<col>><child cols>`, `*` = visited) |
| `f [normal\|fast\|instant]` | the game's fast-mode setting |
| `x <console cmd>` | the game's dev console (`godmode`, `fight <ENCOUNTER>`, `potion <ID>`, `help`...) |
| `t` | debug: every visible clickable control with its node path |

Screens (first word of the reply): `MENU`, `EVENT`, `MAP`, `COMBAT`, `SELECT`, `REWARDS`, `CARD_REWARD`, `CHOOSE_CARD`, `CHOOSE_RELIC`,
`CHOOSE_BUNDLE`, `SHOP`, `RESTSITE`, `TREASURE`, `GAME_OVER`, `MODAL ...`. Any other overlay falls back to a list of its visible buttons
(`[ButtonType] label`). `(busy)` means the game is still animating; ask again with `s`.

## How it works
* `CardSelectCmd.PushSelector` (the game's own test hook) answers every card-choice prompt through `AgentSelector` instead of a UI screen. A Harmony prefix
  (`PromptPatch`) records the prompt and its source ("Choose a card to Discard. (from Survivor)").
* The rest uses the same calls as the game's built-in AutoSlay bot (`MegaCrit.Sts2.Core.AutoSlay`): `ForceClick` on buttons, the `Pressed` signal on card holders,
  `CardModel.TryManualPlay` (the real UI play path), `PotionModel.EnqueueManualUse`, `EndPlayerTurnAction`, `MerchantEntry.OnTryPurchaseWrapper`.
* Settling: after an action the bridge waits for the screen to change, then for a ready combat turn (play phase, empty action queue), a pending card choice, or any
  other screen whose options stay unchanged for 15 frames.
* Tutorials (FTUEs) are turned off on the first command, as AutoSlay does: their popups block flows.
* With mods enabled the game uses a separate `modded/` save profile, so normal saves are untouched.

## Build / dev loop
Needs the .NET 9 SDK. `dotnet build -c Release` copies `AgentBridge.dll` + `AgentBridge.json` into `<game>/mods/AgentBridge/`.
`./dev.sh` closes the game, rebuilds, relaunches through Steam and waits for the bridge.
`python -m agent.autopilot --stop-at-menu` plays a run with trivial rules and reports anomalies (errors, stuck screens, fallback screens).
