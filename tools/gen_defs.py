#!/usr/bin/env python3
"""Generate static card / power definition tables from the decompiled game source.

Usage: tools/gen_defs.py  (writes crates/sts2sim/src/content/gen_cards.rs and gen_powers.rs, prints a report)

Card stat tables (cost, type, rarity, target, keywords, tags, dynamic vars with per-upgrade deltas, upgrade cost /
keyword edits) are extracted mechanically so no number is ever transcribed by hand. Anything the extractor cannot
express is listed in the report and flagged `custom_upgrade` so the card's Rust implementation must handle it.
"""
import glob, os, re, sys, importlib.util

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..")
MODELS = os.path.join(ROOT, "decomp/MegaCrit/Sts2/Core/Models")
OUT = os.path.join(ROOT, "crates/sts2sim/src/content")

spec = importlib.util.spec_from_file_location("gen_ids", os.path.join(HERE, "gen_ids.py"))
# gen_ids runs at import (prints); reuse only slugify by exec of the function source
src = open(os.path.join(HERE, "gen_ids.py")).read()
ns = {}
exec(src[src.index("def slugify"):src.index("def classes")], {"re": re, "os": os}, ns)
slugify = ns["slugify"]


def strip_comments(s):
    s = re.sub(r"/\*.*?\*/", "", s, flags=re.S)
    return re.sub(r"//[^\n]*", "", s)


def read_dir(sub, include_abstract=False):
    out = {}
    for path in sorted(glob.glob(os.path.join(MODELS, sub, "*.cs"))):
        s = strip_comments(open(path, encoding="utf-8-sig").read())
        for m in re.finditer(r"public\s+((?:sealed\s+|abstract\s+|static\s+)*)class\s+(\w+)\s*:\s*(\w+)", s):
            if "static" in m.group(1) or ("abstract" in m.group(1) and not include_abstract):
                continue
            out[m.group(2)] = (m.group(3), s)
    return out


def snake(name):
    return slugify(name)


CARD_TYPE = {"None", "Attack", "Skill", "Power", "Status", "Curse", "Quest"}
TARGET = {"None": "None", "Self": "Self_", "AnyEnemy": "AnyEnemy", "AllEnemies": "AllEnemies", "RandomEnemy": "RandomEnemy",
          "AnyPlayer": "AnyPlayer", "AnyAlly": "AnyAlly", "AllAllies": "AllAllies", "TargetedNoCreature": "TargetedNoCreature",
          "Osty": "Osty"}
KW = {"Exhaust": "kw::EXHAUST", "Ethereal": "kw::ETHEREAL", "Innate": "kw::INNATE", "Unplayable": "kw::UNPLAYABLE",
      "Retain": "kw::RETAIN", "Sly": "kw::SLY", "Eternal": "kw::ETERNAL"}
TAG = {"Strike": "tag::STRIKE", "Defend": "tag::DEFEND", "Minion": "tag::MINION", "OstyAttack": "tag::OSTY_ATTACK", "Shiv": "tag::SHIV"}
PROP = {"Unblockable": 2, "Unpowered": 4, "Move": 8, "SkipHurtAnim": 0x10}
VAR_KIND = {"DamageVar": "Damage", "BlockVar": "Block", "CardsVar": "Cards", "EnergyVar": "Energy", "StarsVar": "Stars",
            "RepeatVar": "Repeat", "HpLossVar": "HpLoss", "HealVar": "Heal", "SummonVar": "Summon", "ForgeVar": "Forge",
            "OstyDamageVar": "OstyDamage", "ExtraDamageVar": "ExtraDamage", "CalculationBaseVar": "CalcBase",
            "CalculationExtraVar": "CalcExtra", "GoldVar": "Gold", "MaxHpVar": "MaxHp", "CalculatedDamageVar": "CalcDamage",
            "CalculatedBlockVar": "CalcBlock"}
PROP_NAME = {"Damage": "Damage", "Block": "Block", "Cards": "Cards", "Energy": "Energy", "Stars": "Stars", "Repeat": "Repeat",
             "HpLoss": "HpLoss", "Heal": "Heal", "Summon": "Summon", "Forge": "Forge", "OstyDamage": "OstyDamage",
             "ExtraDamage": "ExtraDamage", "CalculationBase": "CalcBase", "CalculationExtra": "CalcExtra", "Gold": "Gold", "MaxHp": "MaxHp"}


def num(tok):
    tok = tok.strip().rstrip("mMfF")
    v = float(tok)
    return v


def parse_props(expr):
    bits = 0
    for p in re.findall(r"ValueProp\.(\w+)", expr or ""):
        bits |= PROP.get(p, 0)
    return bits


def find_news(body):
    """Yield (type, generic_arg, args_text) for every `new T<G>(args)` in `body`, including nested ones."""
    for m in re.finditer(r"new\s+(\w+)(?:<(\w+)>)?\(", body):
        i = m.end()
        depth = 1
        j = i
        while j < len(body) and depth:
            if body[j] == "(":
                depth += 1
            elif body[j] == ")":
                depth -= 1
            j += 1
        yield m.group(1), m.group(2), body[i:j - 1]


def main():
    cards = read_dir("Cards")
    powers = read_dir("Powers")
    all_powers = read_dir("Powers", include_abstract=True)
    power_slugs = {snake(n) for n in powers}
    power_by_stem = {n[:-5]: n for n in powers if n.endswith("Power")}
    report = []
    var_names = {}

    # ---- cards ----
    rows = []
    for cls, (base, s) in cards.items():
        if base != "CardModel":
            continue
        slug = snake(cls)
        flags = []
        m = re.search(r"base\(\s*(-?\d+)\s*,\s*CardType\.(\w+)\s*,\s*CardRarity\.(\w+)\s*,\s*TargetType\.(\w+)", s)
        if not m:
            report.append(f"{cls}: cannot parse ctor")
            continue
        cost, ctype, rarity, target = int(m.group(1)), m.group(2), m.group(3), m.group(4)
        x_cost = bool(re.search(r"HasEnergyCostX\s*=>\s*true", s))
        star_x = bool(re.search(r"HasStarCostX\s*=>\s*true", s))
        sm = re.search(r"CanonicalStarCost\s*=>\s*(-?\d+)", s)
        star_cost = int(sm.group(1)) if sm else -1
        if star_x:
            star_cost = -2  # X star cost
        kws = []
        km = re.search(r"CanonicalKeywords\s*=>(.*?);", s, flags=re.S)
        if km:
            kws = [KW[k] for k in re.findall(r"CardKeyword\.(\w+)", km.group(1)) if k in KW]
        tags = []
        tm = re.search(r"CanonicalTags\s*=>(.*?);", s, flags=re.S)
        if tm:
            tags = [TAG[t] for t in re.findall(r"CardTag\.(\w+)", tm.group(1)) if t in TAG]
        mu = re.search(r"MaxUpgradeLevel\s*=>\s*(\d+)", s)
        max_up = int(mu.group(1)) if mu else 1
        turn_end = bool(re.search(r"HasTurnEndInHandEffect\s*=>\s*true", s))
        no_gen = bool(re.search(r"CanBeGeneratedInCombat\s*=>\s*false", s))
        mp_only = bool(re.search(r"MultiplayerConstraint\s*=>\s*CardMultiplayerConstraint\.MultiplayerOnly", s))
        # vars
        vars_ = []  # (kind, arg, base, up, props)
        vm = re.search(r"CanonicalVars\s*=>(.*?);\s*\n", s, flags=re.S)
        if vm:
            body = vm.group(1)
            for name, targ, args in find_news(body):
                if name.startswith("_00"):
                    continue
                if name == "DynamicVar" or name == "IntVar":
                    am = re.match(r'\s*"(\w+)"\s*,\s*([-\d.]+)m?', args)
                    if not am:
                        flags.append(f"var {name}({args})")
                        continue
                    var_names.setdefault(am.group(1), len(var_names))
                    vars_.append(("Named", var_names[am.group(1)], num(am.group(2)), 0, 0, am.group(1)))
                elif name == "PowerVar":
                    am = re.match(r"\s*([-\d.]+)m?", args)
                    pc = targ
                    if not am:
                        flags.append(f"PowerVar<{pc}>({args})")
                        continue
                    if pc not in powers:
                        flags.append(f"PowerVar<{pc}> unknown power")
                        continue
                    vars_.append(("Power", snake(pc), num(am.group(1)), 0, 0, None))
                elif name in VAR_KIND:
                    am = re.match(r"\s*([-\d.]+)m?", args)
                    base_v = num(am.group(1)) if am else 0
                    props = parse_props(args)
                    vars_.append((VAR_KIND[name], 0, base_v, 0, props, None))
                else:
                    flags.append(f"var {name}")
        # upgrade body
        up_cost, up_add, up_rem, up_star = 0, [], [], 0
        custom = False
        um = re.search(r"override\s+void\s+OnUpgrade\(\)\s*\{(.*?)\n\t\}", s, flags=re.S)
        if um:
            body = um.group(1)
            consumed = body
            for u in re.finditer(r"DynamicVars\.(\w+)\.UpgradeValueBy\(\s*(-?[\d.]+)m?\s*\)\s*;", body):
                prop, delta = u.group(1), num(u.group(2))
                consumed = consumed.replace(u.group(0), "")
                target_kind = PROP_NAME.get(prop)
                hit = False
                for i, v in enumerate(vars_):
                    if target_kind and v[0] == target_kind or (v[0] == "Power" and v[1] == snake(prop + "Power")):
                        vars_[i] = (v[0], v[1], v[2], v[3] + delta, v[4], v[5])
                        hit = True
                        break
                if not hit:
                    flags.append(f"upgrade DynamicVars.{prop} has no var")
            for u in re.finditer(r'DynamicVars\["(\w+)"\]\.UpgradeValueBy\(\s*(-?[\d.]+)m?\s*\)\s*;', body):
                nm, delta = u.group(1), num(u.group(2))
                consumed = consumed.replace(u.group(0), "")
                hit = False
                for i, v in enumerate(vars_):
                    if (v[0] == "Named" and v[5] == nm) or (v[0] == "Power" and v[1] == snake(nm)):
                        vars_[i] = (v[0], v[1], v[2], v[3] + delta, v[4], v[5])
                        hit = True
                        break
                if not hit:
                    flags.append(f'upgrade DynamicVars["{nm}"] has no var')
            for u in re.finditer(r"EnergyCost\.UpgradeBy\(\s*(-?\d+)\s*\)\s*;", body):
                up_cost += int(u.group(1))
                consumed = consumed.replace(u.group(0), "")
            for u in re.finditer(r"AddKeyword\(CardKeyword\.(\w+)\)\s*;", body):
                up_add.append(KW[u.group(1)]); consumed = consumed.replace(u.group(0), "")
            for u in re.finditer(r"RemoveKeyword\(CardKeyword\.(\w+)\)\s*;", body):
                up_rem.append(KW[u.group(1)]); consumed = consumed.replace(u.group(0), "")
            for u in re.finditer(r"UpgradeStarCostBy\(\s*(-?\d+)\s*\)\s*;", body):
                up_star += int(u.group(1)); consumed = consumed.replace(u.group(0), "")
            if consumed.strip():
                custom = True
                flags.append("custom OnUpgrade: " + " ".join(consumed.split())[:80])
        rows.append(dict(cls=cls, slug=slug, cost=cost, ctype=ctype, rarity=rarity, target=TARGET[target], x=x_cost,
                         star=star_cost, kws=kws, tags=tags, max_up=max_up, turn_end=turn_end, vars=vars_,
                         up_cost=up_cost, up_add=up_add, up_rem=up_rem, up_star=up_star, custom=custom, flags=flags, no_gen=no_gen, mp_only=mp_only))
        for f in flags:
            report.append(f"{cls}: {f}")
    rows.sort(key=lambda r: r["slug"].encode("ascii"))

    def f(v):
        if abs(v - round(v)) > 1e-9:
            return None
        return int(round(v))

    out = ["// @generated by tools/gen_defs.py from the decompiled game source. Do not edit.",
           "#![allow(clippy::all)]", "use crate::defs::*;", "use crate::ids;", "use crate::types::*;", ""]
    out.append(f"pub static CARD_DEFS: [CardDef; {len(rows)}] = [")
    nonint = []
    for r in rows:
        vs = []
        for kind, arg, base, up, props, nm in r["vars"]:
            b, u = f(base), f(up)
            if b is None or u is None:
                nonint.append(f"{r['cls']}: non-integer var {kind} {base}/{up}")
                b = int(base); u = int(up)
            if kind == "Power":
                vs.append(f"power_var(ids::power::{arg}, {b}, {u})")
            elif kind == "Named":
                vs.append(f"named_var({arg}, {b}, {u})")
            else:
                vs.append(f"var_p(VarKind::{kind}, {b}, {u}, {props})")
        cd = (f"    CardDef::new(ids::card::{r['slug']}, {r['cost']}, CardType::{r['ctype']}, CardRarity::{r['rarity']}, "
              f"TargetType::{r['target']})")
        if r["x"]:
            cd += ".x_cost()"
        if r["star"] != -1:
            cd += f".star_cost({r['star']})"
        if r["kws"]:
            cd += ".kw(" + " | ".join(r["kws"]) + ")"
        if r["tags"]:
            cd += ".tags(" + " | ".join(r["tags"]) + ")"
        if vs:
            cd += ".vars(&[" + ", ".join(vs) + "])"
        if r["max_up"] != 1:
            cd += f".max_upgrade({r['max_up']})"
        if r["up_cost"]:
            cd += f".up_cost({r['up_cost']})"
        if r["up_star"]:
            cd += f".up_star_cost({r['up_star']})"
        if r["up_add"]:
            cd += ".up_add_kw(" + " | ".join(r["up_add"]) + ")"
        if r["up_rem"]:
            cd += ".up_remove_kw(" + " | ".join(r["up_rem"]) + ")"
        if r["turn_end"]:
            cd += ".turn_end_in_hand()"
        if r["custom"]:
            cd += ".custom_upgrade()"
        if r["no_gen"]:
            cd += ".not_generated_in_combat()"
        if r["mp_only"]:
            cd += ".multiplayer_only()"
        out.append(cd + ",")
    out.append("];\n")
    cards_out = out
    report += nonint

    # ---- powers ----
    prow = []
    def resolve(cls, pattern, group=1):
        seen = 0
        while cls in all_powers and seen < 8:
            base, src = all_powers[cls]
            m = re.search(pattern, src)
            if m:
                return m
            cls = base
            seen += 1
        return None

    for cls, (base, s) in powers.items():
        slug = snake(cls)
        t = resolve(cls, r"PowerType\s+Type\s*=>\s*PowerType\.(\w+)")
        st = resolve(cls, r"PowerStackType\s+StackType\s*=>\s*PowerStackType\.(\w+)")
        it = resolve(cls, r"InstanceType\s*=>\s*PowerInstanceType\.(\w+)")
        neg = bool(resolve(cls, r"AllowNegative\s*=>\s*true"))
        sec = bool(resolve(cls, r"OwnerIsSecondaryEnemy\s*=>\s*true"))
        vis = not resolve(cls, r"IsVisibleInternal\s*=>\s*false")
        ptype = t.group(1) if t else None
        if ptype is None and resolve(cls, r"PowerType\s+Type\s*\{[^}]*IsPositive"):
            ip = resolve(cls, r"IsPositive\s*=>\s*(true|false)")
            ptype = "Debuff" if ip and ip.group(1) == "false" else "Buff"
        prow.append(dict(slug=slug, cls=cls, base=base, type=ptype, stack=st.group(1) if st else None,
                         inst=it.group(1) if it else "None", neg=neg, sec=sec, vis=vis))
        if not ptype:
            report.append(f"power {cls}: no Type found (base {base})")
    prow.sort(key=lambda r: r["slug"].encode("ascii"))
    out = ["// @generated by tools/gen_defs.py from the decompiled game source. Do not edit.", "use crate::defs::*;",
           "use crate::types::*;", ""]
    out.append(f"pub static POWER_DEFS: [PowerDef; {len(prow)}] = [")
    for r in prow:
        pt = {"Buff": "Buff", "Debuff": "Debuff", "None": "None"}.get(r["type"], "Buff")
        d = f"    PowerDef::new(PowerType::{pt})"
        if r["neg"]:
            d += ".allow_negative()"
        if r["stack"] and r["stack"] != "Counter":
            d += ".single()"
        if r["inst"] == "Instanced":
            d += ".instanced()"
        elif r["inst"] == "InstancedPerApplier":
            d += ".per_applier()"
        if r["sec"]:
            d += ".secondary_enemy()"
        if not r["vis"]:
            d += ".hidden()"
        out.append(d + ",")
    out.append("];")
    open(os.path.join(OUT, "gen_powers.rs"), "w").write("\n".join(out))


    # ---- card pools (array order matters: it is the RNG source order) ----
    pools_out = ["// @generated by tools/gen_defs.py from the decompiled game source. Do not edit.",
                 "// Card pools in the game's `GenerateAllCards` array order (the order combat generation draws from).",
                 "use crate::ids;", ""]
    for path in sorted(glob.glob(os.path.join(MODELS, "CardPools", "*CardPool.cs"))):
        nm = os.path.basename(path)[:-len("CardPool.cs")]
        if nm in ("Mock", "Deprecated", "Deprived"):
            continue
        src = strip_comments(open(path, encoding="utf-8-sig").read())
        body = src[src.index("GenerateAllCards"):]
        cards_in = re.findall(r"ModelDb\.Card<(\w+)>\(\)", body)
        pools_out.append(f"pub static {slugify(nm)}: [u16; {len(cards_in)}] = [")
        for c in cards_in:
            pools_out.append(f"    ids::card::{snake(c)},")
        pools_out.append("];\n")
    open(os.path.join(OUT, "gen_pools.rs"), "w").write("\n".join(pools_out))

    # ---- potions ----
    pots = read_dir("Potions")
    pr = []
    for cls, (base, src) in pots.items():
        if base != "PotionModel":
            continue
        rar = re.search(r"PotionRarity\s+Rarity\s*=>\s*PotionRarity\.(\w+)", src)
        use = re.search(r"PotionUsage\s+Usage\s*=>\s*PotionUsage\.(\w+)", src)
        tgt = re.search(r"TargetType\s+TargetType\s*=>\s*TargetType\.(\w+)", src)
        gen = not re.search(r"CanBeGeneratedInCombat\s*=>\s*false", src)
        vars_ = []
        vm = re.search(r"CanonicalVars\s*=>(.*?);\s*\n", src, flags=re.S)
        if vm:
            for name, targ, args in find_news(vm.group(1)):
                if name.startswith("_00"):
                    continue
                am = re.match(r"\s*([-\d.]+)m?", args)
                if name == "PowerVar" and am and targ in powers:
                    vars_.append(f"power_var(ids::power::{snake(targ)}, {int(num(am.group(1)))}, 0)")
                elif name in VAR_KIND and am:
                    vars_.append(f"var_p(VarKind::{VAR_KIND[name]}, {int(num(am.group(1)))}, 0, {parse_props(args)})")
                elif name in ("DynamicVar", "IntVar"):
                    nm = re.match(r'\s*"(\w+)"\s*,\s*([-\d.]+)', args)
                    if nm:
                        var_names.setdefault(nm.group(1), len(var_names))
                        vars_.append(f"named_var({var_names[nm.group(1)]}, {int(num(nm.group(2)))}, 0)")
                    else:
                        report.append(f"potion {cls}: var {name}({args})")
                else:
                    report.append(f"potion {cls}: var {name}({args})")
        pr.append(dict(slug=snake(cls), rar=rar.group(1) if rar else "None", use=use.group(1) if use else "None",
                       tgt=TARGET.get(tgt.group(1), "None") if tgt else "None", gen=gen, vars=vars_))
    pr.sort(key=lambda r: r["slug"].encode("ascii"))
    out = ["// @generated by tools/gen_defs.py from the decompiled game source. Do not edit.", "use crate::defs::*;",
           "use crate::ids;", "use crate::types::*;", ""]
    out.append(f"pub static POTION_DEFS: [PotionDef; {len(pr)}] = [")
    for r in pr:
        d = f"    PotionDef::new(ids::potion::{r['slug']}, PotionRarity::{r['rar']}, PotionUsage::{r['use']}, TargetType::{r['tgt']})"
        if r["vars"]:
            d += ".vars(&[" + ", ".join(r["vars"]) + "])"
        if not r["gen"]:
            d += ".not_generated_in_combat()"
        out.append(d + ",")
    out.append("];")
    open(os.path.join(OUT, "gen_potions.rs"), "w").write("\n".join(out))

    vn = ["/// Named (non-standard) dynamic var ids, referenced by `VarKind::Named` `arg`.", "pub mod var_name {"]
    for n, i in sorted(var_names.items(), key=lambda t: t[1]):
        vn.append(f"    pub const {slugify(n)}: u16 = {i};")
    vn.append("}\n")
    cards_out[6:6] = vn
    open(os.path.join(OUT, "gen_cards.rs"), "w").write("\n".join(cards_out))
    print(f"cards: {len(rows)}  powers: {len(prow)}  named vars: {len(var_names)}")
    print(f"custom-upgrade cards: {sum(1 for r in rows if r['custom'])}")
    for line in report:
        print("  -", line)


main()
