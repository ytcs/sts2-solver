# STS2 self-play harness

This repo lets Claude play Slay the Spire 2 through `python -m agent` (tool reference in `README.md`).

Before the first action of any run, invoke the `sts2` skill (root of the strategy book: the rules that never bend, the skill hierarchy, when to load each level), then `sts2-harness`, `sts2-strategy` and `sts2-pathing` (load `sts2-pathing` again and deliberate at every Neow / ancient and before every map click). Load the character and act skills progressively as the run needs them (`sts2-<character>` at character select, `sts2-<character>-act<N>` on entering the act); lower levels only add to or deviate from the general ones. After each run, update the book at the most specific level where the lesson holds (protocol in `sts2`).
