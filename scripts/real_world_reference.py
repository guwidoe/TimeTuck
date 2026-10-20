"""Independent full recomputation for the bounded RealWorld application model.

This reads the original JSON directly, never the flattened Java model or solver
caches. Soft weights use scale 100. Attribute balance deliberately applies to
occupied groups, matching the retained application spike; this is not a claim
of all RealWorld product semantics. Infeasible schedules still receive a score.
"""
from collections import Counter
from decimal import Decimal
from itertools import combinations

SCALE = 100
PAIR_KINDS = {'keep_together', 'keep_apart', 'prefer_together', 'prefer_apart'}


def scaled(value, default=1000):
    number = Decimal(str(default if value is None else value)) * SCALE
    if number != number.to_integral_value():
        raise ValueError('benchmark weights must be exact integers after scaling by 100')
    return int(number)


def sessions(item, count):
    return range(count) if item.get('session_indices') is None else item['session_indices']


def initial_schedule(problem):
    """Fixed attendance first; then most spare seats, last group on ties."""
    scenario = problem['scenario']
    count = scenario['session_count']
    people, groups = scenario['people'], scenario['groups']
    schedule = {f'session_{s}': {g['id']: [] for g in groups} for s in range(count)}
    pins = {}
    for constraint in problem['constraints']:
        if constraint['type'] == 'fixed_placement':
            for person in constraint['person_ids']:
                for session in sessions(constraint, count):
                    key = (person, session)
                    if key in pins and pins[key] != constraint['group_id']:
                        raise ValueError('contradictory fixed placements')
                    pins[key] = constraint['group_id']
    for s in range(count):
        row = schedule[f'session_{s}']
        attending = [p['id'] for p in people if s in sessions(p, count)]
        for p in attending:
            if (p, s) in pins:
                row[pins[p, s]].append(p)
        for p in attending:
            if (p, s) not in pins:
                index = max(range(len(groups)), key=lambda i: (
                    (groups[i].get('session_capacities') or [groups[i]['capacity']] * count)[s]
                    - len(row[groups[i]['id']]), i))
                row[groups[index]['id']].append(p)
    return schedule


def score_schedule(problem, schedule):
    scenario = problem['scenario']
    count = scenario['session_count']
    people = {p['id']: p for p in scenario['people']}
    groups = {g['id']: g for g in scenario['groups']}
    errors, pin_violations = [], []
    locations, members = {}, {}
    capacity_penalty = 0
    expected_sessions = {f'session_{s}' for s in range(count)}
    if set(schedule) != expected_sessions:
        errors.append('session keys differ from scenario')
    for s in range(count):
        row = schedule.get(f'session_{s}', {})
        if set(row) != set(groups):
            errors.append(f'session {s}: group keys differ from scenario')
        for gid, occupants in row.items():
            if gid not in groups:
                errors.append(f'session {s}: unknown group {gid}')
                continue
            members[s, gid] = occupants
            group = groups[gid]
            cap = (group.get('session_capacities') or [group['capacity']] * count)[s]
            capacity_penalty += max(0, len(occupants) - cap)
            for pid in occupants:
                if pid not in people:
                    errors.append(f'session {s}: unknown person {pid}')
                    continue
                if (pid, s) in locations:
                    errors.append(f'session {s}: duplicate person {pid}')
                locations[pid, s] = gid
                if s not in sessions(people[pid], count):
                    errors.append(f'session {s}: absent person {pid} assigned')
        for pid, person in people.items():
            if s in sessions(person, count) and (pid, s) not in locations:
                errors.append(f'session {s}: attending person {pid} missing')
    pairs = Counter()
    for occupants in members.values():
        pairs.update(tuple(sorted(pair)) for pair in combinations(occupants, 2))
    unique_weight, repeat_max, repeat_squared, repeat_weight = 0, 0, False, 0
    for objective in problem['objectives']:
        kind = objective['type']
        if kind == 'maximize_unique_contacts':
            unique_weight = scaled(objective['weight'])
        elif kind == 'avoid_repeats':
            repeat_max = objective['max_encounters_per_pair']
            repeat_squared = objective['penalty_curve'] == 'squared'
            repeat_weight = scaled(objective['weight'])
        else:
            raise ValueError(f'unsupported objective {kind}')
    repeats = sum(max(0, n - repeat_max) ** (2 if repeat_squared else 1) for n in pairs.values())
    soft_pairs = balance_penalty = hard_pairs = 0
    for ci, constraint in enumerate(problem['constraints']):
        kind = constraint['type']
        if kind == 'fixed_placement':
            for pid in constraint['person_ids']:
                for s in sessions(constraint, count):
                    actual = locations.get((pid, s))
                    if actual is not None and actual != constraint['group_id']:
                        pin_violations.append({'constraint': ci, 'person': pid, 'session': s,
                                               'expected': constraint['group_id'], 'actual': actual})
        elif kind in PAIR_KINDS:
            together = kind.endswith('together')
            violations = 0
            for left, right in combinations(constraint['person_ids'], 2):
                for s in sessions(constraint, count):
                    a, b = locations.get((left, s)), locations.get((right, s))
                    if a is not None and b is not None and ((a == b) != together):
                        violations += 1
            if kind.startswith('keep'):
                hard_pairs += violations
            else:
                soft_pairs += violations * scaled(constraint.get('penalty_weight'))
        elif kind == 'attribute_balance':
            for s in sessions(constraint, count):
                for gid in constraint.get('group_ids') or groups:
                    occupants = members.get((s, gid), [])
                    if not occupants:
                        continue  # Explicit sparse/nonempty application-model policy.
                    values = Counter(people[p].get('attributes', {}).get(constraint['attribute_key'])
                                     for p in occupants if p in people)
                    for value, target in constraint['target_counts'].items():
                        gap = target - values[value]
                        gap = max(0, gap) if constraint.get('target_mode') == 'at_least' else abs(gap)
                        balance_penalty += gap * scaled(constraint.get('penalty_weight'))
        else:
            raise ValueError(f'unsupported constraint {kind}')
    hard = -capacity_penalty - hard_pairs
    soft = len(pairs) * unique_weight - repeats * repeat_weight - soft_pairs - balance_penalty
    return {'score': [hard, soft], 'feasible': hard == 0 and not errors and not pin_violations,
            'structural_errors': errors, 'pin_violations': pin_violations,
            'breakdown': {'capacity_penalty': capacity_penalty, 'hard_pair_penalty': hard_pairs,
                          'unique_contacts': len(pairs), 'repeat_penalty_units': repeats,
                          'soft_pair_penalty': soft_pairs, 'balance_penalty': balance_penalty}}


def validate_reported_score(problem, schedule, reported):
    result = score_schedule(problem, schedule)
    result['reported_score'] = reported
    result['score_matches'] = list(reported) == result['score']
    result['valid'] = result['score_matches'] and not result['structural_errors'] and not result['pin_violations']
    return result
