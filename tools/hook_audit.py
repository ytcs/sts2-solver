#!/usr/bin/env python3
"""Static audit: C# hook overrides (decompiled game models) that the Rust listener of the same class does not implement.

  tools/hook_audit.py [--kinds relics,powers,cards,potions,enchantments,afflictions,monsters] [--decomp DIR]

For every `public|protected override <ret> Name(` in decomp/.../Models/<Kind>/<Class>.cs that is a *hook* (an async/Task or
value-returning virtual of AbstractModel and friends, not a property) the snake_case name must appear as a `fn` inside the
matching `listener!(Class { ... })` block. Names that legitimately differ (OnPlay/OnUse/...) are mapped below. The output lists
the gaps, grouped by class, so each can be checked against combat relevance by hand (run-level hooks are expected to be absent).
"""
import argparse, glob, os, re, sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
ap = argparse.ArgumentParser()
ap.add_argument("--kinds", default="relics,powers,cards,potions,enchantments,afflictions")
ap.add_argument("--decomp", default="/home/ytc/Projects/sts2-solver/decomp/MegaCrit/Sts2/Core/Models")
a = ap.parse_args()

# C# overrides that are properties / data, not behaviour hooks, or run-level only (never fire in combat)
SKIP = {
    "IsAllowed", "GetUnlockedRelics", "ShouldReceiveCombatHooks", "SpawnsMerchantItem", "ModifyMerchantPrice",
    "ModifyRewards", "ModifyCardRewardOptions", "ModifyCardRewardCreationOptions", "AfterRoomEntered", "BeforeRoomEntered",
    "AfterObtained", "AfterRemoved", "AfterCardsRewardsGenerated", "ModifyRestSiteOptions", "AfterRestSiteHeal",
    "ModifyRestSiteHealAmount", "ShouldDisableRemainingRestSiteOptions", "TryModifyRestSiteHealRewards",
    "ModifyGeneratedMap", "ModifyMapPointGoldRewards", "ModifyXp", "AfterMapGenerated", "ModifyShopItems",
    "AfterItemPurchased", "AfterPotionProcured", "ModifyPotionRewards", "ShouldForcePotionReward", "ModifyUnknownMapPointRoomTypes",
    "ModifyNextEvent", "ShouldRefillMerchantEntry", "ModifyMerchantCardPool", "ModifyMerchantCardCreationResults",
    "ModifyMerchantCardRarity", "ShouldAllowAncient", "AfterActEntered", "AfterGoldLost", "ModifyOddsIncreaseForUnrolledRoomType",
    "ShouldGenerateTreasure", "ModifyTreasureRoomRewards", "ModifyBossRewards", "ModifyEliteRewards", "ShouldAllowEvent",
    "TryModifyCardRewardAlternatives", "ModifyHealRestSite",
}
MAP = {"OnPlay": "on_play", "OnUse": "on_use_potion", "OnUpgrade": "", "OnPlayWrapper": "", "OnUseWrapper": ""}


def snake(n):
    s = re.sub(r"(?<=[a-z0-9])([A-Z])", r"_\1", n)
    s = re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1_\2", s)
    return s.lower()


def rust_blocks():
    blocks = {}
    for f in glob.glob(os.path.join(ROOT, "crates/sts2sim/src/content/**/*.rs"), recursive=True):
        src = open(f).read()
        for m in re.finditer(r"listener!\((\w+)\s*\{", src):
            i = m.end()
            depth, j = 1, i
            while j < len(src) and depth:
                depth += (src[j] == "{") - (src[j] == "}")
                j += 1
            blocks[m.group(1)] = src[i:j]
    return blocks


blocks = rust_blocks()
kinds = {"relics": "Relics", "powers": "Powers", "cards": "Cards", "potions": "Potions", "enchantments": "Enchantments", "afflictions": "Afflictions", "monsters": "Monsters"}
tot = 0
for k in a.kinds.split(","):
    rows = []
    for f in sorted(glob.glob(os.path.join(a.decomp, kinds[k], "*.cs"))):
        cls = os.path.basename(f)[:-3]
        if cls not in blocks:
            continue  # unported or helper
        src = open(f).read()
        hooks = set()
        for m in re.finditer(r"public override (?:async )?(?:Task<[^>]+>|Task|bool|decimal|int|void|IEnumerable<[^>]+>|\w+\??) (\w+)\(", src):
            hooks.add(m.group(1))
        missing = []
        for h in sorted(hooks):
            if h in SKIP:
                continue
            rn = MAP.get(h, snake(h))
            if rn and not re.search(r"fn\s+" + re.escape(rn) + r"\b", blocks[cls]):
                missing.append(h)
        if missing:
            rows.append((cls, missing))
    print(f"== {k}: {len(rows)} classes with unmapped C# overrides")
    for cls, ms in rows:
        print(f"  {cls}: {', '.join(ms)}")
        tot += len(ms)
print("total gaps:", tot)
