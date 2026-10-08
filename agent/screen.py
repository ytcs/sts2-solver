import re

HEADER = re.compile(r"\bA(\d+) F(\d+)\b")
OPTION = re.compile(r"^(\d+) (.*)")
BARE_NUMBER = re.compile(r"^\d+(\s|$)")


def kind(text):
    return text.split("\n", 1)[0].split(" ")[0] if text else "?"


def busy(text):
    return text.split("\n")[0].endswith("(busy)")


def floor_key(text):
    m = HEADER.search(text or "")
    return f"A{m.group(1)} F{m.group(2)}" if m else None


def act_index(text):
    m = re.search(r"A(\d+) F\d+", text or "")
    return int(m.group(1)) - 1 if m else None


def header_line(text, default=None):
    return next((l for l in (text or "").split("\n") if re.search(r"A\d+ F\d+", l)), default)


def character(text):
    m = re.search(r"\bA\d+ F\d+\s+([A-Z]+)", text or "")
    return m.group(1) if m else None


def hp(text):
    m = re.search(r"HP (\d+)/(\d+)", text or "")
    return (int(m.group(1)), int(m.group(2))) if m else None


def gold(text):
    m = re.search(r"\bG(\d+)\b", text or "")
    return int(m.group(1)) if m else None


def belt(text):
    m = re.search(r"pots\[([^\]]*)\]", text or "")
    return [p.strip() for p in m.group(1).split(",")] if m else []


def options(text):
    return [(m.group(1), m.group(2)) for l in (text or "").split("\n") for m in [OPTION.match(l)] if m]


def option_line(text, num):
    return next((l for l in (text or "").split("\n") if l.startswith(f"{num} ")), "")


def is_bare_number(step):
    return bool(BARE_NUMBER.match(step))


def resolve(state, step):
    if not step.startswith("~"):
        return step
    words = step[1:].split()
    args = []
    while words and (re.fullmatch(r"e\d+|!|\d+", words[-1]) and len(words) > 1):
        args.insert(0, words.pop())
    want = " ".join(words).lower()
    opts = [(n, t.lower()) for n, t in options(state)]
    starts = [n for n, t in opts if t.startswith(want)]
    hits = starts if len(starts) == 1 else [n for n, t in opts if want in t]
    same = {t for n, t in opts if n in hits}
    if len(hits) > 1 and (len(same) == 1 or all(re.fullmatch(r"\d+ gold", t) for t in same)):
        hits = hits[:1]
    if len(hits) == 1:
        return " ".join([hits[0]] + args)
    if len(hits) > 1:
        return f"ERR `{want}` matches options {', '.join(hits)}: name it more exactly, or use the number in its own call"
    return f"ERR no option matching `{want}`"
