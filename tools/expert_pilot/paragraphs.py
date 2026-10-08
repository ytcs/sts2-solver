import json, sys, html
rows = json.load(open(sys.argv[1]))
out, cur, start = [], [], 0.0
for r in rows:
    if cur and r["start"] - start >= 30:
        out.append((start, " ".join(cur)))
        cur, start = [], r["start"]
    if not cur:
        start = r["start"]
    cur.append(html.unescape(r["text"]).replace("\n", " "))
if cur:
    out.append((start, " ".join(cur)))
with open(sys.argv[2], "w", encoding="utf-8") as f:
    for s, t in out:
        s = int(s)
        f.write(f"[{s // 3600}:{s % 3600 // 60:02d}:{s % 60:02d}] {t}\n")
