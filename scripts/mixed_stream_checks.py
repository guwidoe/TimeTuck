#!/usr/bin/env python3
"""Original mixed public-move/score comparisons; no upstream method credit."""
from copy import deepcopy
import random
import subprocess
from differential_scores import ROOT
from differential_lists import apply as apply_list

SEED = 20261108
DOMAIN = (-2, 0, 3)


def valid(case, state):
    basics, lists = state['basic'], state['lists']
    if len(basics) != len(case['basic']) or len(lists) != len(basics):
        raise ValueError('Mixed state shape')
    if any(v is not None and v not in DOMAIN for v in basics):
        raise ValueError('Basic domain')
    if any(pin and value != old for pin, value, old in zip(case['pins'], basics, case['basic'])):
        raise ValueError('Basic pin')
    flat = [v for row in lists for v in row]
    if len(flat) != len(set(flat)) or any(v not in case['values'] for v in flat):
        raise ValueError('List ownership/domain')


def apply(case, state, ops):
    result = deepcopy(state)
    for op in ops:
        kind, *v = op
        if kind == 'b':
            i, value = v
            if not 0 <= i < len(result['basic']):
                raise ValueError('Basic index')
            result['basic'][i] = value
        else:
            rows = result['lists']
            def element(e, i):
                return 0 <= e < len(rows) and 0 <= i < len(rows[e])
            if kind == 'a':
                value, e, i = v
                if not 0 <= e < len(rows) or not 0 <= i <= len(rows[e]):
                    raise ValueError('Insertion index')
            elif kind == 'c':
                e, i, d, j = v
                if not element(e, i) or not 0 <= d < len(rows) or not 0 <= j <= len(rows[d]) - (d == e):
                    raise ValueError('Relocation index')
            elif kind == 's':
                e, i, d, j = v
                if not element(e, i) or not element(d, j):
                    raise ValueError('Swap index')
            elif kind == 'u':
                e, start, end = v
                if not 0 <= e < len(rows) or not 0 <= start <= end <= len(rows[e]):
                    raise ValueError('Unassign span')
            else:
                raise ValueError('Move kind')
            result['lists'] = apply_list(rows, op)
        valid(case, result)
    return result


def score(state):
    hard, soft = 0, 0
    for owner, row in enumerate(state['lists']):
        coefficient = state['basic'][owner] or 0
        hard -= max(0, len(row) - 2)
        soft += coefficient * len(row)
        for index, value in enumerate(row):
            soft += coefficient * value + index
            soft += row[index - 1] if index else 0
            soft -= row[index + 1] if index + 1 < len(row) else 0
    return hard, soft


def states(case):
    current = {'basic': case['basic'][:], 'lists': deepcopy(case['lists'])}
    valid(case, current)
    yield deepcopy(current)
    for batch in case['batches']:
        if batch['mode'] not in ('t', 'c'):
            raise ValueError('Batch mode')
        candidate = apply(case, current, batch['moves'])
        yield deepcopy(candidate)
        if batch['mode'] == 't':
            yield deepcopy(current)
        else:
            current = candidate


def text(value):
    return '-' if value is None else str(value)


def expected(case, index, state):
    locations = {v: (owner, i) for owner, row in enumerate(state['lists']) for i, v in enumerate(row)}
    views = []
    for value in case['values']:
        if value not in locations:
            views.append('-')
        else:
            owner, i = locations[value]
            row = state['lists'][owner]
            views.append(f'{owner}:{i}:{text(row[i-1] if i else None)}:{text(row[i+1] if i+1<len(row) else None)}')
    hard, soft = score(state)
    return '\t'.join([case['id'], str(index), str(int(all(v is not None for v in state['basic']))),
                      str(hard), str(soft), ','.join(map(text, state['basic'])),
                      ';'.join(','.join(map(str, row)) or '-' for row in state['lists']), ','.join(views)])


def validate(case, index, state, line):
    if line != expected(case, index, state):
        raise ValueError(f'Mixed score/state mismatch: {case["id"]} state {index}; expected {expected(case,index,state)!r}, got {line!r}')


def encode(case):
    batches = '|'.join(batch['mode'] + '@' + ';'.join(','.join(map(text, op)) for op in batch['moves'])
                       for batch in case['batches'])
    return '\t'.join([case['id'], ','.join(map(text, case['basic'])), ','.join(str(int(v)) for v in case['pins']),
                      ','.join(map(str, case['values'])) or '-',
                      ';'.join(','.join(map(str, row)) or '-' for row in case['lists']), batches])


def generate():
    cases = []
    def add(basic, pins, values, lists, batches):
        cases.append({'id': f'mixed-{len(cases):03}', 'basic': basic, 'pins': pins, 'values': values, 'lists': lists,
                      'batches': [{'mode': mode, 'moves': moves} for mode, moves in batches]})
    add([0, None], [False, False], [11, 23], [[11], [23]],
        [('t', [['b', 0, 3], ['c', 0, 0, 1, 1]]), ('c', [['b', 1, -2]])])
    add([None], [False], [], [[]], [('t', []), ('c', [['b', 0, 3]]), ('t', [['b', 0, None]])])
    add([3, -2], [True, False], [11, 23, 47, 89], [[11, 23, 47], [89]],
        [('t', [['b', 1, 0], ['s', 0, 0, 1, 0]]), ('c', [['u', 0, 1, 3], ['a', 47, 1, 0]]),
         ('t', [['c', 1, 1, 1, 0], ['b', 1, None]])])
    add([None, None], [False, False], [11, 23], [[], []],
        [('c', [['b', 0, 3], ['b', 0, -2], ['a', 11, 0, 0], ['a', 23, 0, 0]]),
         ('t', [['b', 1, 0], ['u', 0, 0, 2], ['a', 23, 1, 0]])])
    rng = random.Random(SEED)
    while len(cases) < 128:
        count, owners = rng.randrange(7), rng.randint(1, 3)
        values = [11, 23, 47, 89, 131, 193][:count]
        basic = [rng.choice([None, *DOMAIN]) for _ in range(owners)]
        pins = [v is not None and rng.random() < .2 for v in basic]
        lists = [[] for _ in range(owners)]
        shuffled = values[:]; rng.shuffle(shuffled)
        for v in shuffled:
            if rng.random() < .8: lists[rng.randrange(owners)].append(v)
        add(basic, pins, values, lists, [])
        case = cases[-1]; current = {'basic': basic[:], 'lists': deepcopy(lists)}
        for _ in range(12):
            candidate = deepcopy(current); moves = []
            for _ in range(rng.randint(1, 4)):
                options = [['b', i, v] for i, pin in enumerate(pins) if not pin for v in [None, *DOMAIN]]
                rows = candidate['lists']; flat = [v for row in rows for v in row]
                for e, row in enumerate(rows):
                    options += [['a', v, e, i] for v in values if v not in flat for i in range(len(row)+1)]
                    for i in range(len(row)):
                        for d, other in enumerate(rows):
                            options += [['c', e, i, d, j] for j in range(len(other)+1-(d==e)) if (e,i)!=(d,j)]
                            options += [['s', e, i, d, j] for j in range(len(other)) if (e,i)!=(d,j)]
                    options += [['u', e, start, end] for start in range(len(row)) for end in range(start+1,len(row)+1)]
                if not options: break
                op = rng.choice(options); moves.append(op); candidate = apply(case,candidate,[op])
            mode = 't' if rng.random() < .5 else 'c'
            case['batches'].append({'mode': mode, 'moves': moves})
            if mode == 'c': current = candidate
    return cases


def run_checks(classpath):
    cases = generate(); payload = '\n'.join(map(encode, cases)) + '\n'
    commands = {'java': ['java','-Xms256m','-Xmx256m','-cp',classpath,'MixedStreamOracle'],
                'rust': [str(ROOT/'target/release/examples/mixed_stream_oracle')]}
    outputs = {}
    for runtime, command in commands.items():
        run = subprocess.run(command,input=payload,text=True,capture_output=True,timeout=180)
        (ROOT/f'work/mixed-stream-{runtime}.stderr.log').write_text(run.stderr)
        if run.returncode: raise RuntimeError(f'{runtime} mixed oracle failed: {run.stderr[-4000:]}')
        outputs[runtime] = run.stdout.splitlines()
    observations = [(case,i,state) for case in cases for i,state in enumerate(states(case))]
    if any(len(rows) != len(observations) for rows in outputs.values()):
        raise ValueError('Mixed response count mismatch')
    results = []
    for j,(case,i,state) in enumerate(observations):
        for runtime, rows in outputs.items(): validate(case,i,state,rows[j])
        results.append({'id':case['id'],'state':i,'assignment':state,
                        **{runtime:rows[j] for runtime,rows in outputs.items()}})
    return {'seed':SEED,'passed':len(cases),'total':len(cases),'observations':len(observations),
            'operations':sum(len(batch['moves']) for case in cases for batch in case['batches']),
            'cases':cases,'results':results,
            'scope':'Original mixed required-basic/optional-list models, non-dense list identities, pinned basic values, partial states and coupled capacity/load/neighbor scoring. Java public MoveTester executes composed basic and list assign/change/swap/span-unassign operations; SolutionManager.UPDATE_SCORE_ONLY recalculates each observed score. Rust uses mixed transactional streams. Python independently checks ordinary scores, completeness, basic/list assignments and owner/index/previous/next views for initial, candidate, undo and committed states. No Java match analysis, structural-score equivalence, search/termination, custom shadows, list pin/range, incremental Java timing or upstream-method credit.'}
