"""Original public-API gather comparisons; no upstream method credit or timing."""
import json
import random
import subprocess

SEED = 20261212

def generate():
    rng = random.Random(SEED)
    cases = []
    for i in range(256):
        count, owners = rng.randint(1, 9), rng.randint(1, 4)
        initial = [[] for _ in range(owners)]
        for value in rng.sample(range(count), count):
            if rng.random() < .75: initial[rng.randrange(owners)].append(value)
        assigned = [v for row in initial for v in row]
        if i % 4 == 0 and assigned:
            members = rng.sample(assigned, rng.randint(1, len(assigned)))
            entity, boundary = -1, 0
        else:
            members = rng.sample(range(count), rng.randint(1, count))
            entity = rng.randrange(owners)
            boundary = rng.randrange(len(initial[entity])+1)
        cases.append(dict(id=f'gather-{i:03}', count=count, initial=initial,
                          moves=[['g', entity, boundary, *members]]))
    return cases


def compare(commands, work):
    from differential_lists import encode, expected, validate
    cases = generate()
    payload = '\n'.join(map(encode, cases))+'\n'
    outputs = {}
    for runtime, command in commands.items():
        run = subprocess.run(command, input=payload, text=True, capture_output=True, timeout=120)
        (work/f'gather-{runtime}.stderr.log').write_text(run.stderr)
        if run.returncode: raise RuntimeError(f'{runtime} gather failed; see work/oracle/gather-{runtime}.stderr.log')
        outputs[runtime] = run.stdout.splitlines()
        if len(outputs[runtime]) != len(cases): raise ValueError('Gather response count mismatch')
    failures = []
    for i, case in enumerate(cases):
        for runtime, rows in outputs.items():
            error = validate(case, rows[i])
            if error: failures.append(dict(case=case['id'],runtime=runtime,error=error,expected=expected(case),actual=rows[i]))
    failed = {f['case'] for f in failures}
    report = dict(total=len(cases),passed=len(cases)-len(failed),seed=SEED,failures=failures,
                  scope='Original nonempty single gathers: ordered insertion at original boundaries, unassigned members during insertion, and unassignment of assigned members. Candidate/undo/commit lists and four built-in shadows checked against Python. No pins, Java metadata, composite gather sequences, timing or method coverage credit.')
    (work/'list-gather-cases.json').write_text(json.dumps(dict(cases=cases,outputs=outputs,report=report),indent=2)+'\n')
    print(f'List gather: {report["passed"]}/{report["total"]} cases',flush=True)
    return report
