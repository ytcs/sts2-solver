# ranwid — Windows tester guide

`ranwid` is a **live advisor** for an ongoing Slay the Spire 2 run. It watches your normal (unmodded) save
file and, in real time, tells you the best card to remove and whether to take or skip reward cards — graded
against the elites in your current Act. **It only reads the save; it never modifies the game or your run.**

## What you got

A single file: **`ranwid.exe`**. No installer, no .NET install needed — everything is bundled. ~68 MB.

## How to run

1. Start (or continue) a run in Slay the Spire 2 (Ironclad, Silent, Regent, Necrobinder, or Defect — all five).
2. **Double-click `ranwid.exe`.** A full-screen dashboard opens.
3. ranwid finds your save automatically and shows a live, navigable dashboard: a deck-strength bar, your deck,
   the act boss, and the per-elite survival / HP-loss table. Numbers stream in as the solver runs — the screen
   never freezes, and you can move around while it computes. Keys:
   - `↑↓` move the selection · `space` include/exclude the highlighted elite from the current-act strength
   - `r` best card(s) to **remove** · `u` best to **upgrade** · `c` **check a reward** card (type the offered
     card name(s), Tab/typo-tolerant). `+`/`-` step how many cards at once (1–3, e.g. a multi-card event reward).
   - `n` toggle the **next-act projection** (off by default — it roughly doubles the wait; turn it on only when
     weighing a plan-ahead decision, and the strength bar + advice gain a next-act column)
   - `d` refresh · `q` quit
4. As you play (advance screens, fight, pick cards) ranwid notices the save change and refreshes automatically —
   no restart, no keypress needed.

> If Windows SmartScreen warns about an unknown publisher, choose **More info → Run anyway** (the exe is
> unsigned). It is a normal .NET console app.

> **If no save is found**, ranwid shows a panel offering to retry the scan (`r`) or let you type the save folder
> path manually (`m`). See "Auto-detection" below.

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

If auto-detection finds nothing, ranwid shows a **"No run found"** panel. Press `m` to enter the folder, and a
text line opens at the top of the panel:

```
Save folder — paste the SlayTheSpire2 folder (or a current_run.save), then Enter
```

**Paste the full path to your Slay the Spire 2 save folder** and press Enter (or `r` to just rescan). You can
paste the `SlayTheSpire2` folder itself or any parent of it (or a `current_run.save` file directly) — ranwid
searches recursively. Esc cancels the entry.

### Finding your save folder

In File Explorer's address bar, paste `%APPDATA%` (or `%LOCALAPPDATA%`) and press Enter, then look for a
`SlayTheSpire2` folder. The save you want is `current_run.save` somewhere inside it. Copy that folder's path
from the address bar and paste it after pressing `m`.

## Other ways to point ranwid at the save (advanced)

- Command line: `ranwid.exe --save-dir "C:\path\to\SlayTheSpire2"` — search this folder.
- Command line: `ranwid.exe --save "C:\full\path\to\current_run.save"` — read this exact file.
- Environment variable: set `RANWID_SAVE_DIR` to the folder.

## Custom deck mode (no save — for multiplayer guests / deck-building)

In **multiplayer, only the host's game writes a run-save** — if a friend is hosting, your own machine has no
`current_run.save` to read. For that case (and for planning decks generally), use the save-less sandbox:

```
ranwid.exe --custom              # starts from the Ironclad starter deck
ranwid.exe --custom Silent       # …or any of: Ironclad, Silent, Regent, Necrobinder, Defect
```

You get the same dashboard (deck strength + per-elite numbers + removal/upgrade/reward advice). Press `e` to open
the **deck editor** — a command line at the top of the elites panel that takes one command per Enter (Esc closes):

- `+<card>` / `-<card>` — add / remove one copy (a bare card name also adds; typos auto-correct).
- `act <1-4 | name>` — choose which Act's elites to grade against (Underdocks, Overgrowth, Hive, Glory).
- `char <name>` — switch character (resets to its starter deck). `hp <n>`, `asc <n>` — set HP / ascension.
- `reset` — back to the starter deck.

The deck-strength bar and per-elite numbers update live as you edit, and `r` / `u` / `c` give the same
removal / upgrade / reward advice as a real run.

## Please report

Auto-detect now targets the confirmed path `%APPDATA%\SlayTheSpire2\steam\…` first, so it should find your run
without prompting. If it *does* still ask for the folder manually, **tell us the path you ended up pasting**
(the part after your `C:\Users\<you>\` is what we care about) — that means your install differs from the
confirmed layout and helps us add it to auto-detect.
