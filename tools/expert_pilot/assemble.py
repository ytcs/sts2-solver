import sys
f, md = sys.argv[1], sys.argv[2]
s = open(f).read()
rep = {
 "PO_A11": "-0.0110 (0.0053), n 24; both win 100%, end HP 15.3 vs 16.8",
 "VERDICT_A11": "**expert error, small**: R3 1.5 HP-eq at 2.1 se; R1 overstates it ~9x; R2 inconclusive. Low-moderate confidence",
 "PO_B7": "not run: on a fresh 2 s decision the live player chose ST, i.e. agreed with him",
 "VERDICT_B7": "**unresolved**: R1 and R2 disagree in sign, and the live player's own choice flips between runs; no call",
 "PO_D16": "not run",
 "VERDICT_D16": "immaterial (< 1 HP-eq by R2)",
 "PO_D1": "**his +0.0620 (0.0048)**, n 16; both win, end HP 27.7 vs 19.0",
 "VERDICT_D1": "**solver gap**: R2 and R3 agree (R3 8.7 HP-eq, 13 se); R1 had the sign wrong. High confidence; matches his stated reason",
 "PO_E0": "his +0.185 (0.178), n 12; win 92% vs 83%, end HP 19.3 vs 16.7",
 "VERDICT_E0": "**solver gap**: R2 at 3.2 se; R3 same sign, noisy (n 12). Moderate confidence",
 "PO_E1": "not run",
 "VERDICT_E1": "immaterial by R2 (0.25 HP-eq; R1's 6.7 HP-eq not confirmed)",
 "LIVEWORTH_E": "`proposal.fight_objective` -> win only. The live player then plays **DS+ at #0, #1 and #2** (attacks the sleeping boss on turn 1 whatever he has already played) and S before SB+ at #3 (order only); 8/12 agree. Same finding as #0.",
}
for k in sorted(rep, key=len, reverse=True):
    assert k in s, k
    s = s.replace(k, rep[k])
m = open(md).read()
m = m.replace("FIGHTS_PLACEHOLDER", s.strip())
m = m.replace("- Act 1 = Underdocks (catalog act index 1); boss Lagavulin Matriarch.",
 "- Act 1 = Underdocks (records use `act: 0`, the run's act index, as live records do; the simulator does not read `act`); boss Lagavulin Matriarch.")
m = m.replace("B T2 Sneaky Gremlin spawn HP 15 in the simulator vs 12 shown (Merc at <= its split threshold; synced to 12)",
              "B T2 the simulator spawned Sneaky Gremlin at 15/15 vs 12/12 shown (synced to the frame)")
m = m.replace("\n## Caveats", open(sys.argv[3]).read() + "\n## Caveats")
open(md, "w").write(m)
