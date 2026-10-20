"""Flatten a RealWorld problem.json into an integer model file for the Java spike,
using the same start schedule and weight scaling (x100) as the TimeTuck spike.
Also converts a Java result back into a gm-cli evaluate input.

prep.py model <problem.json> <out.txt>
prep.py result <problem.json> <java-result.txt> <settings.json> <out-eval-input.json>
"""
import json, sys
from pathlib import Path
from decimal import Decimal

SCALE = 100


def w(v, default=1000.0):
    scaled = Decimal(str(default if v is None else v)) * SCALE
    if scaled != scaled.to_integral_value():
        raise ValueError("benchmark weight must be integral after scaling by 100")
    return int(scaled)


def mask(indices, n):
    if indices is None:
        return (1 << n) - 1
    m = 0
    for i in indices:
        m |= 1 << i
    return m


def load(path):
    d = json.loads(Path(path).read_text())
    sc = d['scenario']
    S = sc['session_count']
    people = [p['id'] for p in sc['people']]
    groups = [g['id'] for g in sc['groups']]
    pidx = {p: i for i, p in enumerate(people)}
    gidx = {g: i for i, g in enumerate(groups)}
    caps = [[(g.get('session_capacities') or [g['capacity']] * S)[s] for g in sc['groups']] for s in range(S)]
    attends = [set(p.get('session_indices') if p.get('session_indices') is not None else range(S)) for p in sc['people']]
    return d, sc, S, people, groups, pidx, gidx, caps, attends


def model(path, out):
    d, sc, S, people, groups, pidx, gidx, caps, attends = load(path)
    P, G = len(people), len(groups)
    uc, rep = 0, (0, 0, 0)
    for o in d['objectives']:
        if o['type'] == 'maximize_unique_contacts':
            uc = w(o['weight'])
        elif o['type'] == 'avoid_repeats':
            rep = (o['max_encounters_per_pair'], int(o['penalty_curve'] == 'squared'), w(o['weight']))
        else:
            raise SystemExit('unsupported objective ' + o['type'])
    fixed, reqs, bals = {}, [], []
    kinds = {'keep_together': 0, 'keep_apart': 1, 'prefer_together': 2, 'prefer_apart': 3}
    for ci, c in enumerate(d['constraints']):
        t = c['type']
        if t == 'fixed_placement':
            for p in c['person_ids']:
                for s in (c.get('session_indices') if c.get('session_indices') is not None else range(S)):
                    fixed[(pidx[p], s)] = gidx[c['group_id']]
        elif t in kinds:
            ids = [pidx[p] for p in c['person_ids']]
            for i in range(len(ids)):
                for j in range(i + 1, len(ids)):
                    a, b = sorted((ids[i], ids[j]))
                    reqs.append((ci, kinds[t], w(c.get('penalty_weight')) if t.startswith('prefer') else 0, a, b, mask(c.get('session_indices'), S)))
        elif t == 'attribute_balance':
            gm = mask([gidx[g] for g in c['group_ids']] if c.get('group_ids') else None, G)
            targets = list(c['target_counts'].items())
            hits = [(pi, ti) for pi, p in enumerate(sc['people']) for ti, (v, _) in enumerate(targets)
                    if p['attributes'].get(c['attribute_key']) == v]
            bals.append((w(c['penalty_weight']), int(c.get('target_mode') == 'at_least'), mask(c.get('session_indices'), S), gm, [n for _, n in targets], hits))
        else:
            raise SystemExit('unsupported constraint ' + t)
    # Same start as the TimeTuck spike: fixed first, then the group with most spare seats (last on ties, like Rust max_by_key).
    occ = [[0] * G for _ in range(S)]
    var = []
    for s in range(S):
        for p in range(P):
            if s in attends[p]:
                var.append([p, s, fixed.get((p, s), -1), int((p, s) in fixed)])
    for v in var:
        if v[2] >= 0:
            occ[v[1]][v[2]] += 1
    for v in var:
        if v[2] < 0:
            s = v[1]
            g = max(range(G), key=lambda g: (caps[s][g] - occ[s][g], g))
            v[2] = g
            occ[s][g] += 1
    lines = [f'{P} {G} {S}']
    lines += [' '.join(map(str, caps[s])) for s in range(S)]
    lines.append(f'{uc} {rep[0]} {rep[1]} {rep[2]}')
    lines.append(str(len(var)))
    lines += [' '.join(map(str, v)) for v in var]
    lines.append(str(len(reqs)))
    lines += [' '.join(map(str, r)) for r in reqs]
    lines.append(str(len(bals)))
    for wt, al, sm, gm, targets, hits in bals:
        lines.append(f'{wt} {al} {sm} {gm} {len(targets)} ' + ' '.join(map(str, targets)))
        lines.append(f'{len(hits)} ' + ' '.join(f'{p} {t}' for p, t in hits))
    Path(out).write_text('\n'.join(lines) + '\n')


def result(path, res, settings, out):
    d, sc, S, people, groups, *_ = load(path)
    rows = [l.split() for l in Path(res).read_text().splitlines() if l and l[0].isdigit()]
    sched = {f'session_{s}': {g: [] for g in groups} for s in range(S)}
    for p, s, g in ((int(a), int(b), int(c)) for a, b, c in rows):
        sched[f'session_{s}'][groups[g]].append(people[p])
    d.pop('schema_version', None)
    d['incumbent_schedule'] = sched
    d['solver_settings'] = json.loads(Path(settings).read_text())
    Path(out).write_text(json.dumps(d) + '\n')


if __name__ == '__main__':
    {'model': model, 'result': result}[sys.argv[1]](*sys.argv[2:])
