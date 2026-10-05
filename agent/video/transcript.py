"""json3 auto-caption -> compact '[mm:ss] text' lines, one per ~15 s window."""
import json, sys
src, dst = sys.argv[1], sys.argv[2]
win = int(sys.argv[3]) if len(sys.argv) > 3 else 15
ev = json.load(open(src, encoding="utf-8"))["events"]
out, cur_t, buf = [], None, []
for e in ev:
    segs = e.get("segs")
    if not segs: continue
    t = e["tStartMs"] // 1000
    txt = "".join(s.get("utf8", "") for s in segs).replace("\n", " ").strip()
    if not txt: continue
    if cur_t is None or t - cur_t >= win:
        if buf: out.append(f"[{cur_t//60:02d}:{cur_t%60:02d}] " + " ".join(buf))
        cur_t, buf = t, []
    buf.append(txt)
if buf: out.append(f"[{cur_t//60:02d}:{cur_t%60:02d}] " + " ".join(buf))
open(dst, "w", encoding="utf-8").write("\n".join(out))
print(len(out), "lines,", sum(len(l) for l in out), "chars")
