"""Compare an oracle run-replay trace with a live replay log, decision by decision."""
import json, re, sys, collections

def norm(name):
    name = name.strip().rstrip('+')
    return re.sub(r'[^A-Z0-9_]', '', re.sub(r'[ \-]', '_', name.upper()))

def header(screen):
    m = re.search(r'HP (\d+)/(\d+) G(\d+) pots\[(.*?)\]', screen)
    pots = [norm(p) for p in m.group(4).split(', ') if p.strip() != '-']
    return dict(hp=int(m.group(1)), max_hp=int(m.group(2)), gold=int(m.group(3)), pots=pots)

def live_combat(screen):
    o = header(screen)
    m = re.search(r'T(\d+) E(\d+)/(\d+) draw(\d+) disc(\d+) exh(\d+)', screen)
    o.update(turn=int(m.group(1)), energy=int(m.group(2)), draw=int(m.group(4)), disc=int(m.group(5)), exh=int(m.group(6)))
    o['block'] = int(re.search(r'^you b(\d+)', screen, re.M).group(1))
    o['enemies'] = [(int(a), int(b), int(c)) for a, b, c in re.findall(r'^e\d+ .*? (\d+)/(\d+) b(\d+)', screen, re.M) if int(a) > 0]
    hand = []
    for line in screen.split('\n'):
        mm = re.match(r'^\d+ (\(x\) )?(.+?)\((-?\d+|X)\)', line)
        if mm and not line.split(' ', 1)[1].startswith('potion'): hand.append(norm(mm.group(2)))
    o['hand'] = hand
    return o

def mine_combat(r):
    pl = r['player']
    return dict(hp=pl['hp'], max_hp=pl['max_hp'], gold=r['gold'], pots=[p['id'] for p in r['potions']],
                turn=r['turn'], energy=r['energy'], draw=len(r['draw']), disc=len(r['discard']), exh=len(r['exhaust']),
                block=pl['block'], enemies=[(e['hp'], e['max_hp'], e['block']) for e in r['enemies'] if e['hp'] > 0],
                hand=[c['id'] for c in r['hand']])

def hand_eq(live, mine):
    if len(live) != len(mine): return False
    return all(m == l or m.startswith(l + '_') for l, m in zip(live, mine))

def main(trace, live):
    T = [json.loads(l) for l in open(trace)]
    L = [json.loads(l) for l in open(live)]
    dec = [d for d in L if d['event'] == 'decision']
    lc = collections.defaultdict(list); lr = {}; lmap = [d for d in L if d['event'] == 'map']
    for d in dec:
        kind = d['screen'].split('\n')[0].split()[0]
        if kind == 'COMBAT': lc[d['floor']].append(d)
        if kind == 'REWARDS' and d['floor'] not in lr: lr[d['floor']] = d
    mc = collections.defaultdict(list); mr = {}; floor = 1; maps = []
    for r in T:
        ev = r.get('event')
        if ev == 'map': floor = r['floor']
        elif ev == 'combat': mc[floor].append(r)
        elif ev == 'rewards': mr[r['floor']] = r
        if r.get('map'): maps.append(r['map'])
    ok = bad = 0
    if maps and lmap:
        same = maps[0] == lmap[0]['text']
        print('act-1 map identical:', same); ok += same; bad += not same
    for f in sorted(mc):
        recs = [r for r in mc[f]]
        lives = lc.get(f, [])
        n = min(len(lives), len(recs))
        fight_bad = 0
        for j in range(n):
            a, b = live_combat(lives[j]['screen']), mine_combat(recs[j])
            diffs = {k: (a[k], b[k]) for k in a if k != 'hand' and a[k] != b[k]}
            if not hand_eq(a['hand'], b['hand']): diffs['hand'] = (a['hand'], b['hand'])
            if diffs:
                fight_bad += 1
                if fight_bad <= 3: print(f'F{f} decision {j} ({lives[j]["action"]}): {diffs}')
        over = recs[-1].get('combat_over') if recs else None
        print(f'F{f} combat: {n - fight_bad}/{n} decisions match; live {len(lives)} decisions, oracle {len(recs)} records, over={over}')
        ok += n - fight_bad; bad += fight_bad
    for f in sorted(mr):
        if f not in lr: print(f'F{f} rewards: no live screen'); continue
        scr = lr[f]['screen']
        live_items = []
        for line in scr.split('\n')[2:]:
            m = re.match(r'^\d+ (.*)$', line)
            if not m or m.group(1).startswith('proceed'): continue
            s = m.group(1)
            if s.startswith('card: '): live_items.append(('card', [norm(x) for x in s[6:].split(' | ')]))
            elif s.startswith('potion '): live_items.append(('potion', norm(s[7:].split(':')[0])))
            elif re.match(r'^\d+ Gold', s): live_items.append(('gold', int(s.split()[0])))
            elif s.startswith('relic '): live_items.append(('relic', norm(s[6:].split(':')[0])))
            else: live_items.append(('?', s))
        mine = []
        for rw in mr[f]['offered']['rewards']:
            t = rw['type']
            mine.append((t, rw.get('amount') if t == 'gold' else rw.get('id') if t in ('potion', 'relic') else [c['id'] for c in rw.get('cards', [])]))
        same = live_items == mine
        print(f'F{f} rewards {"match" if same else "DIFFER"}: live={live_items}' + ('' if same else f' oracle={mine}'))
        ok += same; bad += not same
    # every act map
    lm = [d['text'] for d in lmap]
    for i, m in enumerate(maps):
        same = any(m == t for t in lm)
        print(f'map {i}: {"identical to a live map event" if same else "NOT in live maps"}'); ok += same; bad += not same
    # shops: first screen per floor vs oracle stock before the first purchase
    for d in dec:
        if not d['screen'].startswith('SHOP'): continue
        f = d['floor']
        first = next(x for x in dec if x['floor'] == f and x['screen'].startswith('SHOP'))
        if d is not first: continue
        live = []
        for line in d['screen'].split('\n')[2:]:
            m = re.match(r'^\d+ (\d+)g (card|relic|potion) (.+?)(\(|:)', line)
            if m: live.append((m.group(2), norm(m.group(3)), int(m.group(1))))
            m = re.match(r'^\d+ (\d+)g remove a card', line)
            if m: live.append(('remove', None, int(m.group(1))))
        sh = next(r for r in T if r.get('event') == 'shop' and r['floor'] == f)
        mine = [(k, norm(v) if isinstance(v, str) else None, e['cost']) if k != 'remove' else ('remove', None, e['remove'])
                for e in sh['stock'] for k, v in [next((kk, e[kk]) for kk in ('card', 'relic', 'potion', 'remove') if kk in e)]]
        same = len(live) == len(mine) and all(a[0] == b[0] and a[2] == b[2] and (a[1] == b[1] or (a[1] or '').startswith((b[1] or '') + '_')) for a, b in zip(live, mine))
        print(f'F{f} shop {"match" if same else "DIFFER"} ({len(live)} entries)' + ('' if same else f'\n  live={live}\n  mine={mine}')); ok += same; bad += not same
    for d in dec:
        if d['screen'].startswith('TREASURE') and d['action'].startswith('TREASURE'):
            m = re.search(r'^0 take (.+?):', d['screen'], re.M)
            tr = next((r for r in T if r.get('event') == 'treasure' and r['floor'] == d['floor']), None)
            same = tr is not None and m is not None and [norm(m.group(1))] == tr['relics']
            print(f'F{d["floor"]} treasure relic {"match" if same else "DIFFER"}: live={m and m.group(1)} oracle={tr and tr["relics"]}'); ok += same; bad += not same
    end = T[-1]
    if end.get('event') == 'end':
        st = end['state']
        print('final oracle state:', st['player']['hp'], '/', st['player']['max_hp'], 'gold', st['gold'])
    print(f'live decisions: {len(dec)}')
    print(f'TOTAL match={ok} mismatch={bad}')

main(sys.argv[1], sys.argv[2])
