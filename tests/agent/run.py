"""Runs the harness tests without pytest: `python tests/agent/run.py [-k substring] [-x]`.

Discovers `test_*` functions in `tests/agent/test_*.py` and passes the fixtures they name: `monkeypatch` (support.MonkeyPatch, undone after the test) and
`tmp_path` (a fresh pathlib directory). The same files run under pytest unchanged.
"""
import importlib
import inspect
import os
import pathlib
import shutil
import sys
import tempfile
import time
import traceback

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.abspath(os.path.join(HERE, "..", "..")))

import support  # noqa: E402


def main(argv):
    sys.stdout.reconfigure(encoding="utf-8")
    pat = argv[argv.index("-k") + 1] if "-k" in argv else None
    first_fail = "-x" in argv
    mods = sorted(f[:-3] for f in os.listdir(HERE) if f.startswith("test_") and f.endswith(".py"))
    passed, failed, skipped = 0, [], 0
    t_all = time.time()
    for m in mods:
        mod = importlib.import_module(m)
        for name, fn in inspect.getmembers(mod, inspect.isfunction):
            if not name.startswith("test_") or fn.__module__ != m:
                continue
            label = f"{m}::{name}"
            if pat and pat not in label:
                continue
            mp = support.MonkeyPatch()
            tmp = tempfile.mkdtemp(prefix="sts2t_")
            kw = {}
            params = inspect.signature(fn).parameters
            if "monkeypatch" in params:
                kw["monkeypatch"] = mp
            if "tmp_path" in params:
                kw["tmp_path"] = pathlib.Path(tmp)
            t0 = time.time()
            try:
                fn(**kw)
                passed += 1
                print(f"PASS {label} ({time.time() - t0:.1f}s)")
            except Exception as e:  # noqa: BLE001
                if type(e).__name__ in ("Skip", "Skipped"):
                    skipped += 1
                    print(f"SKIP {label}: {e}")
                else:
                    failed.append(label)
                    print(f"FAIL {label}\n" + traceback.format_exc())
            finally:
                mp.undo()
                shutil.rmtree(tmp, ignore_errors=True)
            if failed and first_fail:
                break
        if failed and first_fail:
            break
    print(f"\n{passed} passed, {len(failed)} failed, {skipped} skipped in {time.time() - t_all:.0f}s")
    for f in failed:
        print("  failed:", f)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
