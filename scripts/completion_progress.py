"""Transparent readiness index; never an estimate of remaining effort or time."""
import hashlib
import json
import math
from pathlib import Path

DEFAULT_POLICY = {'weights': {'features': 40, 'behavior': 20, 'performance': 30, 'evidence': 10},
                  'java_time_multiplier': 1}
BEHAVIOR_JOBS = ('differential', 'solver_differential', 'list_differential', 'stream_differential')
PERFORMANCE_JOBS = ('performance', 'solver_performance', 'mixed_performance', 'heuristic_performance',
                    'construction_performance', 'repair_performance', 'list_repair_performance',
                    'list_performance', 'kopt_performance', 'stream_performance', 'concurrent_performance',
                    'sequence_performance', 'aggregate_performance', 'pillar_performance')
UPDATE_JOBS = frozenset(('concurrent_performance', 'sequence_performance', 'aggregate_performance', 'pillar_performance'))
ALL_JOBS = BEHAVIOR_JOBS + PERFORMANCE_JOBS


def _number(value):
    if not isinstance(value, (int, float)) or isinstance(value, bool):
        return False
    try:
        return math.isfinite(value)
    except (OverflowError, ValueError):
        return False


def _count(value):
    return isinstance(value, int) and not isinstance(value, bool) and value >= 0


def _policy(policy):
    if policy is not None and not isinstance(policy, dict):
        raise ValueError('Completion policy must be a mapping.')
    policy = policy or {}
    if not isinstance(policy.get('weights', {}), dict):
        raise ValueError('Completion weights must be a mapping.')
    weights = {**DEFAULT_POLICY['weights'], **policy.get('weights', {})}
    target = policy.get('java_time_multiplier', 1)
    if (set(weights) != set(DEFAULT_POLICY['weights']) or
            any(not _number(v) or v < 0 for v in weights.values()) or sum(weights.values()) != 100 or
            not _number(target) or target <= 0):
        raise ValueError('Completion weights must total 100; Java time multiplier must be positive and finite.')
    return {'weights': weights, 'java_time_multiplier': target}


def model_digest(policy=None):
    """Separate presentation-model key; does not change verification/benchmark keys."""
    encoded = json.dumps(_policy(policy), sort_keys=True, separators=(',', ':'), allow_nan=False).encode()
    return hashlib.sha256(Path(__file__).read_bytes() + b'\0' + encoded).hexdigest()


def _ratio(check):
    if not isinstance(check, dict):
        return None
    total, passed = check.get('total'), check.get('passed')
    if not _count(total) or total == 0 or not _count(passed) or passed > total:
        return None
    return passed / total


def _fresh(job):
    return job.get('status', 'unknown') if isinstance(job, dict) and job.get('status') in ('current', 'stale') else 'unknown'


def _item(name, ratio, status, freshness, detail, **extra):
    return {'name': str(name)[:160], 'percent': ratio * 100, 'status': status,
            'freshness': freshness, 'detail': detail[:400], **extra}


def _features(report):
    items, ratios = [], []
    families = report.get('families')
    if not isinstance(families, list):
        return items, ratios
    for index, family in enumerate(families[:256]):
        family = family if isinstance(family, dict) else {}
        total, excluded, passed = family.get('total'), family.get('excluded', 0), family.get('passed')
        name = family.get('name', f'Family {index + 1}')
        if not _count(total) or not _count(excluded) or excluded > total:
            items.append(_item(name, 0, 'unknown', 'unknown', 'Missing or invalid family counts.'))
            ratios.append(0)
            continue
        scope = total - excluded
        if scope == 0:
            continue
        valid = _count(passed) and passed <= scope
        ratio = min(1, max(0, passed / scope)) if valid else 0
        ratios.append(ratio)
        items.append(_item(name, ratio, 'met' if ratio == 1 else 'not_met' if valid else 'unknown',
                           'current' if isinstance(report.get('verification'), dict) and report['verification'].get('successful') is True else 'unknown',
                           f'{passed if valid else "unknown"}/{scope} in-scope assertions passed.',
                           met=passed if valid else 0, total=scope))
    if len(families) > 256:
        ratios.append(0)
        items.append(_item('Additional families', 0, 'unknown', 'unknown', 'Family limit exceeded.'))
    return items, ratios


def _behavior(report):
    items, ratios = [], []
    for name in BEHAVIOR_JOBS:
        job = report.get(name)
        job = job if isinstance(job, dict) else {}
        checks = [('top-level', job)] + [(key, value) for key, value in job.items()
                                        if isinstance(value, dict) and ('total' in value or 'passed' in value)]
        values = [(key, _ratio(value)) for key, value in checks]
        worst, ratio = min(values, key=lambda pair: -1 if pair[1] is None else pair[1])
        ratios.append(ratio if ratio is not None else 0)
        items.append(_item(name, ratio or 0, 'unknown' if ratio is None else 'met' if ratio == 1 else 'not_met',
                           _fresh(job), f'Worst count check: {worst}; Java errors/unsupported remain in total.',
                           met=checks[[key for key, _ in checks].index(worst)][1].get('passed', 0) if ratio is not None else 0,
                           total=checks[[key for key, _ in checks].index(worst)][1].get('total', 0) if ratio is not None else 0))
    return items, ratios


def _score(value):
    return isinstance(value, list) and bool(value) and all(_number(part) for part in value)


def _samples(row):
    captured = row.get('samples')
    if not isinstance(captured, dict):
        return None
    result = []
    for runtime in ('rust', 'java'):
        values = captured.get(runtime)
        if not isinstance(values, list) or not values:
            return None
        for value in values:
            elapsed = value.get('elapsed_ns') if isinstance(value, dict) else value
            if not _number(elapsed) or elapsed <= 0:
                return None
        result.append(values)
    return result if len(result[0]) == len(result[1]) else None


def _quality(name, job, row, samples):
    if name == 'performance':
        return (True, 'Captured score-arithmetic checksum.') if _number(row.get('checksum')) else (None, 'Missing checksum.')
    if name in UPDATE_JOBS:
        validation = _ratio(job.get('validation'))
        if validation != 1:
            return (None if validation is None else False), 'Update validation is missing or incomplete.'
        entries = samples[0] + samples[1]
        key = next((key for key in ('checksum', 'result') if all(isinstance(v, dict) and key in v for v in entries)), None)
        if key is None:
            return None, 'Missing captured checksum/result agreement.'
        values = [v[key] for v in entries]
        if any(v is None for v in values) or (key == 'checksum' and any(not _number(v) for v in values)):
            return None, 'Invalid captured checksum/result.'
        try:
            encoded = [json.dumps(v, sort_keys=True, allow_nan=False, separators=(',', ':')) for v in values]
        except (TypeError, ValueError, OverflowError):
            return None, 'Invalid captured checksum/result.'
        return (True, 'Full validation and captured results agree.') if len(set(encoded)) == 1 else (False, 'Captured results differ.')
    if name == 'solver_performance':
        # This saved pipeline independently enumerates the optimum, validates each
        # native/Java output, then stores numeric elapsed samples only.
        return (True, 'Exact-solver pipeline optimum and captured runtimes.') if _score(row.get('optimal_score')) else (None, 'Missing validated optimum.')
    rust, java = row['rust']['score'] if isinstance(row.get('rust'), dict) and 'score' in row['rust'] else None, row['java']['score'] if isinstance(row.get('java'), dict) and 'score' in row['java'] else None
    if not _score(rust) or not _score(java) or len(rust) != len(java):
        return None, 'Missing or incompatible numeric score vectors.'
    for expected, captures in zip((rust, java), samples):
        for sample in captures:
            if isinstance(sample, dict) and 'score' in sample:
                actual = sample['score']
                if not _score(actual) or len(actual) != len(expected) or actual != expected:
                    return None, 'Captured score and summary disagree or have incompatible shapes.'
    return (True, 'Native score is equal or better lexicographically.') if tuple(rust) >= tuple(java) else (False, 'Native solution quality is worse.')


def _row(name, job, row, target):
    row = row if isinstance(row, dict) else {}
    title = str(row.get('name', 'Unnamed workload'))[:160]
    rust = row.get('rust') if isinstance(row.get('rust'), dict) else {}
    java = row.get('java') if isinstance(row.get('java'), dict) else {}
    rm, jm = rust.get('median'), java.get('median')
    samples = _samples(row)
    if not _number(rm) or not _number(jm) or rm <= 0 or jm <= 0 or samples is None:
        return _item(title, 0, 'unknown', _fresh(job), 'Missing or invalid captured runtimes/medians.')
    for runtime in (rust, java):
        if any(key in runtime and (not _number(runtime[key]) or runtime[key] <= 0) for key in ('min', 'max')):
            return _item(title, 0, 'unknown', _fresh(job), 'Invalid runtime range.')
        if runtime.get('min', runtime['median']) > runtime['median'] or runtime.get('max', runtime['median']) < runtime['median']:
            return _item(title, 0, 'unknown', _fresh(job), 'Inconsistent runtime range.')
    quality, detail = _quality(name, job, row, samples)
    ratio = min(1, jm / rm * target) if quality is True else 0
    return _item(title, ratio, 'unknown' if quality is None else 'met' if ratio == 1 else 'not_met',
                 _fresh(job), detail, rust_median=rm, java_median=jm)


def _performance(report, target):
    items, ratios = [], []
    for name in PERFORMANCE_JOBS:
        job = report.get(name)
        job = job if isinstance(job, dict) else {}
        workloads = job.get('workloads')
        rows = [_row(name, job, row, target) for row in workloads] if isinstance(workloads, list) else []
        if not rows:
            ratios.append(0)
            items.append(_item(name, 0, 'unknown', _fresh(job), 'No measured comparable workloads.', met=0, total=0))
            continue
        worst = min(rows, key=lambda row: (row['percent'], row['status'] != 'unknown'))
        ratio = worst['percent'] / 100
        ratios.append(ratio)
        met = sum(row['status'] == 'met' for row in rows)
        status = 'met' if met == len(rows) else 'not_met' if any(row['status'] == 'not_met' for row in rows) else 'unknown'
        items.append(_item(name, ratio, status, _fresh(job), f'Worst workload: {worst["name"]}. {worst["detail"]}',
                           met=met, total=len(rows), bottleneck=worst))
    return items, ratios


def build_completion(report, policy=None):
    """Build bounded JSON output without mutating report or evidence freshness."""
    report = report if isinstance(report, dict) else {}
    policy = _policy(policy)
    current = sum(_fresh(report.get(name)) == 'current' for name in ALL_JOBS)
    stale = [name for name in ALL_JOBS if _fresh(report.get(name)) == 'stale']
    verification = report.get('verification')
    gate = isinstance(verification, dict) and verification.get('successful') is True
    summary = report.get('summary')
    clean = isinstance(summary, dict) and all(_count(summary.get(key)) and summary[key] == 0
                                              for key in ('failed', 'unverified'))
    features = _features(report)
    behavior = _behavior(report)
    performance = _performance(report, policy['java_time_multiplier'])
    evidence_items = [_item(name, int(_fresh(report.get(name)) == 'current'),
                            'met' if _fresh(report.get(name)) == 'current' else 'not_met',
                            _fresh(report.get(name)), 'Freshness supplied by the verified report loader.') for name in ALL_JOBS]
    components = []
    definitions = [('features', 'Feature coverage', features, 'Equal-weight in-scope families; exclusions are outside the denominator.'),
                   ('behavior', 'Behavior comparisons', behavior, 'Four jobs, each using its worst top-level/first-level passed/total check.'),
                   ('performance', 'Java time and quality', performance, 'Fourteen jobs; each contributes its worst quality-gated Java/native median ratio.'),
                   ('evidence', 'Current evidence', (evidence_items, [current / 18 if gate and clean else 0]),
                    f'{current}/18 jobs current; full verification successful: {gate}; raw failed/unverified clear: {clean}.')]
    for name, label, (items, ratios), detail in definitions:
        ratio = sum(ratios) / len(ratios) if ratios else 0
        weight = policy['weights'][name]
        components.append({'id': name, 'label': label, 'weight': weight, 'percent': ratio * 100,
                           'earned_points': ratio * weight, 'detail': detail,
                           'met': sum(item['status'] == 'met' for item in items), 'total': len(items), 'items': items})
    ready = gate and clean and current == 18 and all(c['percent'] == 100 and c['total'] > 0 and c['met'] == c['total'] for c in components)
    blockers = [f'{c["label"]}: target not fully demonstrated.' for c in components if c['percent'] < 100]
    if not gate:
        blockers.append('Successful full verification is required.')
    if not clean:
        blockers.append('Raw failed/unverified compatibility status is missing or not clear.')
    return {'schema_version': 1, 'model_digest': model_digest(policy), 'index_percent': sum(c['earned_points'] for c in components),
            'ready': ready, 'weights': policy['weights'], 'target': {'java_time_multiplier': policy['java_time_multiplier']},
            'components': components, 'current_jobs': current, 'total_jobs': 18, 'stale_jobs': stale,
            'blockers': blockers, 'methodology': [
                'This is an explicit readiness/progress index, not remaining effort, time or a general performance guarantee.',
                'Feature families and measurement families have equal internal weight; fastest workloads cannot hide slower workloads.',
                'Missing or invalid evidence earns zero. Native-only reference passes never replace Java comparison passes.',
                'Stale reports retain last-demonstrated progress and remain labelled stale; evidence currentness is scored separately.',
                'Performance means Java-time parity or better at equal-or-better quality on the measured workloads only.']}
