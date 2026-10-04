"""Original round-robin value construction comparison using public Community APIs."""
from differential_solver import generate, score
from pooled_construction_checks import run_cases


def generate_cases():
    return generate() + [
        dict(id='queue-best-target', value_count=2, initial=[None]*3, pins=[False]*3,
             edges=[], weights=[[3, 2], [10, 9], [3, 2]]),
        dict(id='queue-wrap', value_count=2, initial=[None]*5, pins=[False]*5,
             edges=[], weights=[[0, 0]]*5),
        dict(id='queue-all-pinned', value_count=2, initial=[1, 0], pins=[True]*2,
             edges=[(0, 1)], weights=[[1, 2], [3, 4]]),
    ]


def reference(case):
    """Materialize all receiver assignments; cyclic values, stable max ties."""
    if 'domains' in case or 'optional' in case:
        raise ValueError('comparison requires homogeneous required domains')
    values = [v if pin else None for v, pin in zip(case['initial'], case['pins'])]
    if any(pin and v is None for pin, v in zip(case['pins'], values)):
        raise ValueError('required pinned value is missing')
    if case['value_count'] <= 0:
        raise ValueError('empty required domain')
    turn = 0
    while None in values:
        value = turn % case['value_count']
        trials = []
        for i, current in enumerate(values):
            if current is None:
                trial = values.copy()
                trial[i] = value
                trials.append(trial)
        values = max(trials, key=lambda v: score(case, v))
        turn += 1
    return values, score(case, values)


def run_checks(classpath):
    return run_cases(classpath, 'value-queue', generate_cases(), reference,
                     '67 original homogeneous required graph cases with pins and signed weights. '
                     'Public Java ALLOCATE_TO_VALUE_FROM_QUEUE, original entity/value order and NEVER early pick; '
                     'native cycling value queue. Every returned assignment/score is independently validated; '
                     'native assignments also match full receiver reconstruction. Final equal scores and exact '
                     'optima are separate counts. No Java trace, heterogeneous/optional/list/mixed comparison, '
                     'dedicated timing or upstream-method credit.')


if __name__ == '__main__':
    from prepare_solver_oracle import prepare_solver_java
    cp, _ = prepare_solver_java()
    raise SystemExit(bool(run_checks(cp)['failures']))
