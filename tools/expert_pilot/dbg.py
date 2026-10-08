import importlib.util, json, sys
sys.path.insert(0, sys.argv[0].rsplit("\\", 1)[0].rsplit("/", 1)[0])
from build import Builder
m = importlib.util.spec_from_file_location("spec", sys.argv[1]); mod = importlib.util.module_from_spec(m); m.loader.exec_module(mod)
b = Builder(mod.SPEC)
orig = b.act
def act(a, nxt, where):
    orig(a, nxt, where)
    snap = json.loads(b.sim.snapshot())
    print(where, b.sim.stage(), [(e["id"][-4:], e["hp"], e.get("alive"), e.get("block")) for e in snap["enemies"]], "P", snap["player"]["hp"], snap["player"]["block"])
b.act = act
try:
    b.run()
except Exception as ex:
    print("ERR", ex)
