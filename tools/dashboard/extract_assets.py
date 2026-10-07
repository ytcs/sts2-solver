"""Extract the dashboard's art from the game's pack, once (not part of the polling loop):

    python tools/dashboard/extract_assets.py [--pck PATH] [--out target/dashboard/assets] [--no-vendor]

Reads `SlayTheSpire2.pck` (Godot 4; parsed as untrusted data, nothing in it is executed) and writes, keyed by our ids (UPPER_SNAKE, as in
`data/catalog.json` and `crates/sts2sim/src/ids.rs`):
  cards/<ID>.webp       card portraits (art only: the dashboard draws the frame)
  relics/<ID>.png, potions/<ID>.webp, powers/<ID>.png, orbs/<ID>.webp, intents/<name>.webp, ui/<name>.png (map icons, energy, star, top bar)
  creatures/<ID>/       Spine 4.2 rig of a monster / character: skeleton.skel, skeleton.atlas (pages renamed to .webp), page images, meta.json
                        (skin, animation, scale from the creature's scene); the page draws one still frame of it, in the browser
  manifest.json         every id -> its image and display name (localization/eng), plus the counts
  vendor/spine-canvas.js  the Spine canvas runtime (downloaded once from unpkg; --no-vendor skips it and the page falls back to the CDN)
Needs Pillow (decodes the BC1/BC3/BC7 textures and resizes).
"""
import argparse
import io
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, HERE)
import pck  # noqa: E402

DEFAULT_PCK = r"C:\Program Files (x86)\Steam\steamapps\common\Slay the Spire 2\SlayTheSpire2.pck"
SPINE_URL = "https://unpkg.com/@esotericsoftware/spine-canvas@4.2.120/dist/iife/spine-canvas.js"
CHARACTERS = ("IRONCLAD", "SILENT", "DEFECT", "REGENT", "NECROBINDER", "OSTY")
PARTS = {"CRUSHER": "KAISER_CRAB_BOSS", "ROCKET": "KAISER_CRAB_BOSS"}
UI_SPRITES = {  # ui/<name>.png <- AtlasTexture resources
    **{f"map_{k}": f"images/atlases/ui_atlas.sprites/map/icons/map_{k}.tres" for k in
       ("monster", "elite", "rest", "shop", "unknown", "chest", "chest_boss", "unknown_monster", "unknown_elite", "unknown_shop", "unknown_chest")},
    **{f"energy_{c}": f"images/atlases/ui_atlas.sprites/card/energy_{c}.tres" for c in ("ironclad", "silent", "defect", "regent", "necrobinder", "colorless", "quest")},
    **{k: f"images/atlases/ui_atlas.sprites/top_bar/{k}.tres" for k in ("top_bar_heart", "top_bar_gold", "top_bar_floor", "top_bar_deck", "top_bar_map", "top_bar_ascension")},
}


def snake(cls):
    return re.sub(r"(?<=[a-z0-9])(?=[A-Z])|(?<=[A-Z])(?=[A-Z][a-z])", "_", cls).lower()


def ids_table(path):
    """{module: [(ID, ClassName)]} from crates/sts2sim/src/ids.rs."""
    src = open(path, encoding="utf-8").read()
    out = {}
    for m in re.finditer(r"pub mod (\w+) \{(.*?)\n\}", src, re.S):
        body = m.group(2)
        names = re.search(r"NAMES: \[&str; COUNT\] = \[(.*?)\];", body, re.S)
        classes = re.search(r"CLASS_NAMES: \[&str; COUNT\] = \[(.*?)\];", body, re.S)
        if names and classes:
            out[m.group(1)] = list(zip(re.findall(r'"([^"]+)"', names.group(1)), re.findall(r'"([^"]+)"', classes.group(1))))
    return out


def plain(text):
    """Localization text without markup: [gold]x[/gold] -> x, {Var:fmt()} -> X, energy icons -> E."""
    text = re.sub(r"\{[^{}]*energyIcons[^{}]*\}", "E", text or "")
    text = re.sub(r"\{([A-Za-z]+)[^{}]*\}", "X", text)
    return re.sub(r"\[/?[a-z_]+(=[^\]]*)?\]", "", text).strip()


class Extractor:
    def __init__(self, pack, out):
        self.P = pack
        self.out = out
        self.errors = []
        self._atlas = {}

    def path(self, *parts):
        p = os.path.join(self.out, *parts)
        os.makedirs(os.path.dirname(p), exist_ok=True)
        return p

    def texture(self, src):
        """(kind, payload) of a source texture (`images/.../x.png`) through its .import, or None."""
        imp = self.P.imported(src)
        if not imp:
            return None
        try:
            return pck.ctex(self.P.read(imp))
        except Exception as e:  # noqa: BLE001  one bad texture must not stop the extraction
            self.errors.append(f"{src}: {e}")
            return None

    def save(self, src, dest, max_side=None, fmt=None):
        """Save a source texture as `dest` (relative to the output). WebP blobs are copied as they are unless they need a resize. Returns dest or None."""
        t = self.texture(src)
        if t is None:
            return None
        kind, payload = t
        fmt = fmt or os.path.splitext(dest)[1][1:]
        if kind == fmt and max_side is None:
            with open(self.path(dest), "wb") as f:
                f.write(payload)
            return dest
        im = pck.to_image(kind, payload)
        if max_side and max(im.size) > max_side:
            s = max_side / max(im.size)
            im = im.resize((max(1, round(im.size[0] * s)), max(1, round(im.size[1] * s))), resample=3)
        im.save(self.path(dest), **({"quality": 82, "method": 4} if fmt == "webp" else {"optimize": True}))
        return dest

    def sprite(self, tres, dest):
        """An AtlasTexture resource (`.tres`: atlas path + region) cropped out of its atlas."""
        if tres not in self.P.files:
            return None
        txt = self.P.text(tres)
        a = re.search(r'path="res://([^"]+)"', txt)
        r = re.search(r"region = Rect2\(([-\d.]+), ([-\d.]+), ([-\d.]+), ([-\d.]+)\)", txt)
        if not (a and r):
            return None
        if a.group(1) not in self._atlas:
            t = self.texture(a.group(1))
            self._atlas[a.group(1)] = pck.to_image(*t) if t else None
        im = self._atlas[a.group(1)]
        if im is None:
            return None
        x, y, w, h = (round(float(v)) for v in r.groups())
        im.crop((x, y, x + w, y + h)).save(self.path(dest), optimize=True)
        return dest

    def loc(self, table):
        name = f"localization/eng/{table}.json"
        return json.loads(self.P.text(name)) if name in self.P.files else {}

    # ------------------------------------------------------------ per kind

    def cards(self, catalog, ids):
        loc = self.loc("cards")
        index = {}
        for n in self.P.files:
            m = re.match(r"images/packed/card_portraits/(.+)\.png\.import$", n)
            if m:
                base = m.group(1).rsplit("/", 1)[-1]
                beta = "/beta/" in n
                if base not in index or (index[base][1] and not beta):
                    index[base] = (n[:-len(".import")], beta)
        cls = dict(ids.get("card", []))
        out = {}
        for group, cards in catalog["cards"].items():
            for c in cards:
                cid = c["id"]
                src = next((index[k][0] for k in (cid.lower(), snake(cls.get(cid, ""))) if k in index), None)
                img = self.save(src, f"cards/{cid}.webp", max_side=320) if src else None
                out[cid] = dict(img=img, title=loc.get(f"{cid}.title") or cid.replace("_", " ").title(), type=c.get("type"), rarity=c.get("rarity"),
                                color=group.lower(), cost=c.get("cost"), x=c.get("x", False), desc=plain(loc.get(f"{cid}.description", "")))
        return out

    def simple(self, names, table, folder, src_of, ext, max_side):
        loc = self.loc(table)
        out = {}
        for cid in names:
            src = next((s for s in src_of(cid) if self.P.imported(s)), None)
            img = self.save(src, f"{folder}/{cid}.{ext}", max_side=max_side) if src else None
            out[cid] = dict(img=img, title=loc.get(f"{cid}.title") or cid.replace("_", " ").title(), desc=plain(loc.get(f"{cid}.description", "")))
        return out

    def creature(self, cid, cls):
        """A Spine rig: creature scene -> SpineSprite (skin, animation, scale) -> skeleton data -> atlas + skeleton + pages."""
        scene = next((s for s in (f"scenes/creature_visuals/{cid.lower()}.tscn", f"scenes/creature_visuals/{snake(cls)}.tscn") if s in self.P.files), None)
        if not scene:
            return None
        txt = self.P.text(scene)
        ext = {i: p for p, i in re.findall(r'\[ext_resource type="SpineSkeletonDataResource"[^\]]*path="res://([^"]+)" id="([^"]+)"\]', txt)}
        nodes = re.findall(r'\[node name="([^"]+)" type="SpineSprite"[^\]]*\]\n(.*?)(?=\n\[|\Z)', txt, re.S)
        nodes.sort(key=lambda n: n[0] != "Visuals")
        for name, body in nodes:
            ref = re.search(r'skeleton_data_res = ExtResource\("([^"]+)"\)', body)
            if ref and ref.group(1) in ext:
                break
        else:  # a placeholder creature: a plain Sprite2D with a texture
            tex = {i: p for p, i in re.findall(r'\[ext_resource type="Texture2D"[^\]]*path="res://([^"]+)" id="([^"]+)"\]', txt)}
            spr = re.search(r'\[node name="Visuals" type="Sprite2D"[^\]]*\]\n(.*?)(?=\n\[|\Z)', txt, re.S)
            ref = spr and re.search(r'texture = ExtResource\("([^"]+)"\)', spr.group(1))
            img = self.save(tex[ref.group(1)], f"creatures/{cid}/still.png", max_side=320) if ref and ref.group(1) in tex else None
            return dict(img=img, scene=scene) if img else None
        prop = lambda k: (re.search(rf'^{k} = "?([^"\n]*)"?$', body, re.M) or [None, None])[1]  # noqa: E731
        scale = re.search(r"^scale = Vector2\(([-\d.]+), ([-\d.]+)\)", body, re.M)
        data = self.P.text(ext[ref.group(1)])
        atlas_src = re.search(r'type="SpineAtlasResource"[^\]]*path="res://([^"]+)"', data)
        skel_src = re.search(r'type="SpineSkeletonFileResource"[^\]]*path="res://([^"]+)"', data)
        if not (atlas_src and skel_src):
            return None
        atlas_imp, skel_imp = self.P.imported(atlas_src.group(1)), self.P.imported(skel_src.group(1))
        if not (atlas_imp and skel_imp):
            return None
        atlas_text = json.loads(self.P.text(atlas_imp)).get("atlas_data", "")
        skel = self.P.read(skel_imp)
        if skel_src.group(1).endswith(".json"):
            return None  # JSON skeletons are not used by this build; skip rather than guess
        adir = os.path.dirname(atlas_src.group(1))
        lines, pages = atlas_text.replace("\r", "").split("\n"), []
        for i, line in enumerate(lines):  # a page name is the line after a blank line (or the first line) and ends in an image extension
            if (i == 0 or not lines[i - 1].strip()) and re.search(r"\.(png|jpg|webp)$", line.strip()):
                page = line.strip()
                t = self.texture(f"{adir}/{page}")
                if t is None:
                    return None
                kind, payload = t
                new = re.sub(r"\.\w+$", ".webp" if kind == "webp" else ".png", page)
                with open(self.path("creatures", cid, new), "wb") as f:
                    if kind in ("webp", "png"):
                        f.write(payload)
                    else:
                        buf = io.BytesIO()
                        payload.save(buf, "PNG")
                        f.write(buf.getvalue())
                lines[i] = new
                pages.append(new)
        with open(self.path("creatures", cid, "skeleton.atlas"), "w", encoding="utf-8", newline="\n") as f:
            f.write("\n".join(lines))
        with open(self.path("creatures", cid, "skeleton.skel"), "wb") as f:
            f.write(skel)
        meta = dict(skin=prop("preview_skin"), animation=prop("preview_animation"), scale=float(scale.group(1)) if scale else 1.0, pages=pages,
                    scene=scene, version=skel[9:9 + skel[8] - 1].decode("ascii", "replace") if len(skel) > 16 else None)
        with open(self.path("creatures", cid, "meta.json"), "w", encoding="utf-8") as f:
            json.dump(meta, f)
        return dict(dir=f"creatures/{cid}", **meta)

    def run(self, catalog, ids, vendor=True):
        man = dict(version=None)
        man["cards"] = self.cards(catalog, ids)
        relic_ids = [r["id"] for rs in catalog["relics"].values() for r in rs] + [i for i, _ in ids.get("relic", [])]
        man["relics"] = self.simple(dict.fromkeys(relic_ids), "relics", "relics", lambda i: [f"images/relics/{i.lower()}.png", f"images/relics/beta/{i.lower()}.png"], "png", 96)
        pot_ids = [p["id"] for ps in catalog["potions"].values() for p in ps] + [i for i, _ in ids.get("potion", [])]
        man["potions"] = self.simple(dict.fromkeys(pot_ids), "potions", "potions", lambda i: [f"images/potions/large/{i.lower()}.png", f"images/potions/{i.lower()}.png"], "webp", 96)
        man["powers"] = self.simple([i for i, _ in ids.get("power", [])], "powers", "powers", lambda i: [f"images/powers/{i.lower()}.png"], "png", 64)
        man["orbs"] = self.simple([i for i, _ in ids.get("orb", [])], "orbs", "orbs", lambda i: [f"images/orbs/{i.lower()}.png"], "webp", 100)
        intents = {}
        for n in self.P.files:
            m = re.match(r"images/packed/intents/(?:attack/)?(intent_\w+)\.png\.import$", n)
            if m:
                intents[m.group(1)] = self.save(n[:-len(".import")], f"intents/{m.group(1)}.webp", max_side=96)
        man["intents"] = intents
        man["ui"] = {k: self.sprite(v, f"ui/{k}.png") for k, v in UI_SPRITES.items()}
        man["ui"]["orb_empty"] = self.save("images/orbs/empty_orb.png", "ui/orb_empty.webp")
        man["ui"]["star_icon"] = self.save("images/packed/sprite_fonts/star_icon.png", "ui/star_icon.png")  # Regent star costs (card_data.py writes it too)
        mon_loc, enc_loc = self.loc("monsters"), self.loc("encounters")
        creatures = {}
        for cid, cls in list(ids.get("monster", [])) + [(c, c.title()) for c in CHARACTERS]:
            try:
                c = self.creature(cid, cls)
            except Exception as e:  # noqa: BLE001
                self.errors.append(f"creature {cid}: {e}")
                c = None
            creatures[cid] = dict(c or {}, title=mon_loc.get(f"{cid}.name") or cid.replace("_", " ").title())
        man["creatures"] = creatures
        boss_icons = {}
        for n in self.P.files:
            m = re.match(r"images/atlases/ui_atlas\.sprites/.*?/(\w+)_boss_icon\.tres$", n) or re.match(r"images/map/placeholder/(\w+)_boss_icon\.png\.import$", n)
            if m and f"{m.group(1).upper()}_BOSS" not in boss_icons:
                key = f"{m.group(1).upper()}_BOSS"
                boss_icons[key] = self.sprite(n, f"ui/boss_{key}.png") if n.endswith(".tres") else self.save(n[:-len(".import")], f"ui/boss_{key}.png", max_side=128)
        for part, boss in PARTS.items():  # parts of a boss rig drawn as one: show the boss's map icon
            if part in creatures and not creatures[part].get("dir") and not creatures[part].get("img") and boss_icons.get(boss):
                creatures[part]["img"] = boss_icons[boss]
        man["encounters"] = {e["id"]: dict(title=enc_loc.get(f"{e['id']}.title") or e["id"].replace("_", " ").title(), room=e.get("room"), act=e.get("act"),
                                          icon=boss_icons.get(e["id"])) for e in catalog.get("encounters", [])}
        man["vendor"] = None
        if vendor:
            try:
                import urllib.request
                with urllib.request.urlopen(SPINE_URL, timeout=30) as r:
                    js = r.read()
                if b"SkeletonRenderer" in js:
                    with open(self.path("vendor", "spine-canvas.js"), "wb") as f:
                        f.write(js)
                    man["vendor"] = "vendor/spine-canvas.js"
            except Exception as e:  # noqa: BLE001  offline: the page falls back to the CDN, then to text
                self.errors.append(f"vendor spine-canvas: {e}")
        count = lambda d: sum(1 for v in d.values() if v and (v.get("img") or v.get("dir")))  # noqa: E731
        man["counts"] = {k: f"{count(man[k])}/{len(man[k])}" for k in ("cards", "relics", "potions", "powers", "orbs", "creatures")}
        man["counts"]["intents"] = len([v for v in intents.values() if v])
        man["errors"] = self.errors[:200]
        with open(self.path("manifest.json"), "w", encoding="utf-8") as f:
            json.dump(man, f, indent=0)
        return man


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--pck", default=DEFAULT_PCK)
    ap.add_argument("--out", default=os.path.join(ROOT, "target", "dashboard", "assets"))
    ap.add_argument("--no-vendor", action="store_true", help="do not download the Spine canvas runtime (the page then loads it from the CDN)")
    a = ap.parse_args(argv)
    with open(os.path.join(ROOT, "data", "catalog.json"), encoding="utf-8") as f:
        catalog = json.load(f)
    ids = ids_table(os.path.join(ROOT, "crates", "sts2sim", "src", "ids.rs"))
    pack = pck.Pack(a.pck)
    try:
        man = Extractor(pack, a.out).run(catalog, ids, vendor=not a.no_vendor)
    finally:
        pack.close()
    print(f"assets in {a.out}")
    for k, v in man["counts"].items():
        print(f"  {k}: {v}")
    if man["errors"]:
        print(f"  {len(man['errors'])} problems (first: {man['errors'][0]}); all in manifest.json")
    missing = [k for k, v in man["creatures"].items() if not v.get("dir") and not v.get("img")]
    if missing:
        print(f"  creatures without art (text): {', '.join(missing)}")


if __name__ == "__main__":
    main()
