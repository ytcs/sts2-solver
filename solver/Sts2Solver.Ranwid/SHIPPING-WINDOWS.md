# ranwid — Windows tester guide

`ranwid` is a **live advisor** for an ongoing Slay the Spire 2 run. It watches your normal (unmodded) save
file and, in real time, tells you the best card to remove and whether to take or skip reward cards — graded
against the elites in your current Act. **It only reads the save; it never modifies the game or your run.**

## What you got

A single file: **`ranwid.exe`**. No installer, no .NET install needed — everything is bundled. ~68 MB.

## How to run

1. Start (or continue) an Ironclad run in Slay the Spire 2. (Only Ironclad is supported right now.)
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

## Please report

Auto-detect now targets the confirmed path `%APPDATA%\SlayTheSpire2\steam\…` first, so it should find your run
without prompting. If it *does* still ask for the folder manually, **tell us the path you ended up pasting**
(the part after your `C:\Users\<you>\` is what we care about) — that means your install differs from the
confirmed layout and helps us add it to auto-detect.
