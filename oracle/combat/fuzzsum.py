#!/usr/bin/env python3
"""Summarise errors of an `oracle fuzz` run: groups failing scenarios by first game frames."""
import json, glob, collections, sys
d = sys.argv[1]
c = collections.defaultdict(list)
for f in glob.glob(d + '/*.scenario.json'):
    o = json.load(open(f))
    if 'error' not in o: continue
    e = o['error'].split('\n')
    frames = [l.strip()[3:90] for l in e if l.strip().startswith('at ')][:2]
    c[(e[0][:110],) + tuple(frames)].append(o['name'])
for k, v in sorted(c.items(), key=lambda kv: -len(kv[1])):
    print(len(v), v[:2]); [print('    ', x) for x in k]
