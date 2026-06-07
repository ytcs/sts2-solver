# ranwid — Windows tester guide

`ranwid` is a **live advisor** for an ongoing Slay the Spire 2 run. It watches your normal (unmodded) save
file and, in real time, tells you the best card to remove and whether to take or skip reward cards — graded
against the elites in your current Act. **It only reads the save; it never modifies the game or your run.**

## What you got

A single file: **`ranwid.exe`**. No installer, no .NET install needed — everything is bundled. ~68 MB.

## How to run

1. Start (or continue) a run in Slay the Spire 2 (Ironclad, Silent, Regent, Necrobinder, or Defect — all five).
2. **Double-click `ranwid.exe`.** A console window opens.
3. ranwid finds your save automatically, prints your deck and per-elite stats, and drops into a prompt:
   ```
   ranwid>
   ```
   - Type the cards a reward screen offers (e.g. `Bludgeon Inflame Whirlwind`) to get a take-vs-skip verdict.
     Tab auto-completes card names; small typos auto-correct.
   - `cuts` — best card to remove from your deck.
   - `deck` — per-elite stats for the current deck.
   - `help` — command list. `quit` — exit.
4. As you play (advance screens, fight, pick cards) ranwid notices the save change and refreshes on the next
   prompt — no restart needed.

> If Windows SmartScreen warns about an unknown publisher, choose **More info → Run anyway** (the exe is
> unsigned). It is a normal .NET console app.

## Auto-detection — what it does

When it starts, ranwid looks for the most-recently-updated `current_run.save`. It checks the **confirmed**
Slay the Spire 2 save location first, then a set of fallbacks (each probed for a `SlayTheSpire2` /
`Slay the Spire 2` folder and searched recursively):

- `%APPDATA%\SlayTheSpire2\steam\<steamid>\…`  ← the confirmed save path (mirror of the Linux layout)
- `%APPDATA%`  (……\AppData\Roaming)
- `%LOCALAPPDATA%`  (……\AppData\Local)
- `%USERPROFILE%`, plus `Documents` and `Saved Games`

If that turns up nothing, it falls back to finding Steam (via the registry `HKCU\Software\Valve\Steam\SteamPath`
and `libraryfolders.vdf`) and scans Steam's per-user `userdata` tree and the game install folder too.

Modded runs (anything under a `modded` folder) and `.backup` files are ignored on purpose.

## If it asks for the folder manually

If auto-detection finds nothing, ranwid prints every location it tried (marking which exist) and then asks:

```
Save folder>
```

**Paste the full path to your Slay the Spire 2 save folder** and press Enter. You can paste the `SlayTheSpire2`
folder itself or any parent of it — ranwid searches recursively. It validates that a `current_run.save` actually
lives under what you typed before continuing, and **remembers your choice** for next time (stored in
`%APPDATA%\ranwid\save-dir.txt`).

Leave the line blank and press Enter to quit.

### Finding your save folder

In File Explorer's address bar, paste `%APPDATA%` (or `%LOCALAPPDATA%`) and press Enter, then look for a
`SlayTheSpire2` folder. The save you want is `current_run.save` somewhere inside it. Copy that folder's path
from the address bar and paste it at the `Save folder>` prompt.

## Other ways to point ranwid at the save (advanced)

- Command line: `ranwid.exe --save-dir "C:\path\to\SlayTheSpire2"` — search this folder.
- Command line: `ranwid.exe --save "C:\full\path\to\current_run.save"` — read this exact file.
- Environment variable: set `RANWID_SAVE_DIR` to the folder.

Run `ranwid.exe --once` for a one-shot report (no interactive prompt).

## New full-screen dashboard (experimental): `--tui`

`ranwid.exe --tui` opens a full-screen, navigable dashboard instead of the prompt. It updates live as the solver
runs (numbers fill in without freezing) and keeps the deck and deck-strength panels on screen at all times. It can
also project your deck onto the **next act**, but that roughly doubles the wait, so it's **off by default** — turn
it on with `n` only when you're weighing a plan-ahead decision. Keys:

- `↑↓` move · `space` include/exclude an elite from the current-act strength
- `r` best card(s) to remove · `u` best to upgrade · `c` check a reward card — `+`/`-` steps how many cards (1–3,
  e.g. for a multi-card event reward); multi-card picks list one card per line
- `n` toggle the next-act projection (off by default; when on, the strength bar and advice gain a **next-act** column)
- `d` refresh · `q` quit

It is **experimental** — if anything looks wrong, fall back to the normal `ranwid.exe` (the default prompt is
unchanged). Please report any glitches.

## Custom deck mode (no save — for multiplayer guests / deck-building)

In **multiplayer, only the host's game writes a run-save** — if a friend is hosting, your own machine has no
`current_run.save` to read. For that case (and for planning decks generally), use the save-less sandbox:

```
ranwid.exe --custom              # starts from the Ironclad starter deck
ranwid.exe --custom Silent       # …or any of: Ironclad, Silent, Regent, Necrobinder, Defect
```

You get the same dashboard (deck strength + per-elite numbers), and a prompt to edit the deck by hand so it
mirrors your real run:

- `+<card>` or `add <card> [xN]` — add a card (Tab completes, typos auto-correct). A bare card name also adds.
- `-<card>` or `rm <card>` — remove one copy.
- `act <1-4 | name>` — choose which Act's elites to grade against (Underdocks, Overgrowth, Hive, Glory).
- `char <name>` — switch character (resets to its starter deck). `hp <n>`, `asc <n>` — set HP / ascension.
- `r` — best cards to remove. `c <card…>` — take-vs-skip a reward. `reset` — back to the starter. `help`, `q`.

## Please report

Auto-detect now targets the confirmed path `%APPDATA%\SlayTheSpire2\steam\…` first, so it should find your run
without prompting. If it *does* still ask for the folder manually, **tell us the path you ended up pasting**
(the part after your `C:\Users\<you>\` is what we care about) — that means your install differs from the
confirmed layout and helps us add it to auto-detect.
