"""Grab frames from a downloaded video at given timestamps; optionally tile them into a contact sheet.

  python -m agent.video.frames VIDEO OUTDIR t1 t2 ...            # one jpg per timestamp
  python -m agent.video.frames VIDEO OUTDIR t1 t2 ... --sheet 2x2 --w 960   # tiles of N frames per sheet

Timestamps: seconds, mm:ss or h:mm:ss. Needs ffmpeg on PATH (and Pillow for --sheet).
"""
import argparse, os, subprocess


def secs(s: str) -> float:
    p = [float(x) for x in s.split(":")]
    t = 0.0
    for x in p:
        t = t * 60 + x
    return t


def grab(video, t, path, w):
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-ss", str(t), "-i", video, "-frames:v", "1",
                    "-vf", f"scale={w}:-2", "-q:v", "3", path], check=True)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("video"); ap.add_argument("out"); ap.add_argument("times", nargs="+")
    ap.add_argument("--w", type=int, default=1280, help="frame width")
    ap.add_argument("--sheet", help="CxR tiling, e.g. 2x2")
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    paths = []
    for ts in a.times:
        t = secs(ts)
        p = os.path.join(a.out, f"f{int(t):05d}.jpg")
        grab(a.video, t, p, a.w)
        paths.append((ts, p))
    if not a.sheet:
        print("\n".join(p for _, p in paths)); return
    from PIL import Image, ImageDraw
    c, r = (int(x) for x in a.sheet.split("x"))
    n = c * r
    for i in range(0, len(paths), n):
        chunk = paths[i:i + n]
        ims = [Image.open(p) for _, p in chunk]
        w, h = ims[0].size
        sheet = Image.new("RGB", (w * c, h * r))
        for k, (im, (ts, _)) in enumerate(zip(ims, chunk)):
            ImageDraw.Draw(im).text((6, 4), ts, fill=(255, 255, 0))
            sheet.paste(im, ((k % c) * w, (k // c) * h))
        out = os.path.join(a.out, f"sheet_{chunk[0][0].replace(':', '-')}.jpg")
        sheet.save(out, quality=85)
        print(out)


if __name__ == "__main__":
    main()
