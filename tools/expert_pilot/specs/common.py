AL = dict(S="STRIKE_SILENT", D="DEFEND_SILENT", N="NEUTRALIZE", SV="SURVIVOR", AB="ASCENDERS_BANE", DS="DAGGER_SPRAY", DA="DASH",
          HD="HIDDEN_DAGGERS", CL="CLUMSY", SB="SNAKEBITE", SH="SHIV", ST="STRANGLE", NF="NOXIOUS_FUMES", OB="OUTBREAK", DF="DEFLECT",
          BT="BULLET_TIME")
SOURCE = dict(video="https://youtu.be/hMrQSndDvPc", channel="Baalorlord", uploaded="2026-10-08", title="New Poison Deck!",
              run_seed="YMY1KELG18SC", build="v0.111.0 (2026.08.14)", build_hash="1568834832", modded="MODDED (2)",
              character="SILENT", ascension=10)


def deck(*items):
    out = []
    for tok, n in items:
        up = tok.endswith("+")
        cid = AL.get(tok.rstrip("+"), tok.rstrip("+"))
        c = {"id": cid, "upgrade": int(up)}
        if cid == "DAGGER_SPRAY":
            c["enchantment"] = {"id": "GLAM", "amount": 1}
        out += [dict(c) for _ in range(n)]
    return out
