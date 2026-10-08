import shlex


class Args:
    def __init__(self, argline, valued=(), flags=()):
        toks = shlex.split(argline)
        self.items, self.pos = [], []
        i = 0
        while i < len(toks):
            t = toks[i]
            if t in valued:
                self.items.append((t, toks[i + 1]))
                i += 1
            elif t in flags:
                self.items.append((t, True))
            else:
                self.pos.append(t)
            i += 1

    def has(self, name):
        return any(k == name for k, _ in self.items)

    def get(self, name, default=None, cast=str):
        vals = [v for k, v in self.items if k == name]
        return cast(vals[-1]) if vals else default

    def hp(self, name="--hp", default="full"):
        return hp_value(self.get(name, default))

    def weights(self, name="--w"):
        v = self.get(name)
        return {kv.split("=")[0]: float(kv.split("=")[1]) for kv in v.split(",")} if v else {}


def hp_value(v):
    return v if v in ("full", "current") else int(v)
