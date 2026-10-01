#!/usr/bin/env python3
"""Compare original generated boundary/random cases with the pinned Java public API."""
import hashlib
import json
import platform
import random
import subprocess
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REF = json.loads((ROOT / 'compat/upstream.json').read_text())
WORK = ROOT / 'work/oracle'
JAR = WORK / f'timefold-solver-core-{REF["tag"][1:]}.jar'


def prepare_java():
    WORK.mkdir(parents=True, exist_ok=True)
    url = f'https://repo.maven.apache.org/maven2/ai/timefold/solver/timefold-solver-core/{REF["tag"][1:]}/{JAR.name}'
    if not JAR.exists():
        urllib.request.urlretrieve(url, JAR)
    expected = urllib.request.urlopen(url + '.sha1').read().decode().strip().split()[0]
    if hashlib.sha1(JAR.read_bytes()).hexdigest() != expected:
        raise RuntimeError('Pinned Maven artifact checksum mismatch')
    subprocess.run(['javac', '-cp', str(JAR), '-d', str(WORK), str(ROOT / 'compat/oracle/ScoreOracle.java')], check=True)
    return {'artifact': url, 'sha256': hashlib.sha256(JAR.read_bytes()).hexdigest()}


def source_digest():
    paths = sorted((ROOT / 'crates/solver').rglob('*.rs'))
    paths += sorted((ROOT / 'compat/oracle').glob('*.java'))
    paths += sorted((ROOT / 'scripts').glob('*.py'))
    paths += [ROOT / 'compat/oracle/pom.xml']
    paths += [ROOT / 'Cargo.lock', ROOT / 'Cargo.toml', ROOT / 'crates/solver/Cargo.toml']
    h = hashlib.sha256()
    for p in paths:
        h.update(str(p.relative_to(ROOT)).encode())
        h.update(p.read_bytes())
    return h.hexdigest()


def generate():
    rng = random.Random(20261001)
    cases = []
    edge = [-2**63, -2**63 + 1, -2**53-1, -100, -1, 0, 1, 100, 2**53+1, 2**63-1]
    for kind, labels in [('simple', ['']), ('hard_soft', ['hard', 'soft']), ('hard_medium_soft', ['hard', 'medium', 'soft'])]:
        def score(values, structural=0):
            return (f'{structural}structural/' if structural else '') + '/'.join(f'{x}{label}' for x, label in zip(values, labels))
        for i in range(200):
            a = score([rng.choice(edge) if i < 80 else rng.randint(-1000000, 1000000) for _ in labels], rng.choice([-5, -1, 0, 1]))
            b = score([rng.choice(edge) for _ in labels], rng.choice([-1, 0]))
            for op in ['parse', 'add', 'sub', 'neg', 'abs', 'cmp', 'mul', 'div', 'pow']:
                rhs = b if op in ['add', 'sub', 'cmp'] else rng.choice(['-2', '-0.0', '0', '0.5', '1', '1.2', '2', 'NaN', 'Infinity', '-Infinity'])
                # Cross-runtime transcendental rounding is outside this initial contract.
                if op == 'pow':
                    rhs = rng.choice(['0', '1', '2'])
                cases.append({'type': kind, 'operation': op, 'a': a, 'b': rhs})
        for structural in [-1, 0, 1]:
            cases.append({'type': kind, 'operation': 'parse', 'a': score([0] * len(labels), structural), 'b': ''})
    for hard, soft in [(1,2),(2,3),(0,0),(0,2),(2,0)]:
        kind = f'bendable_{hard}_{soft}'
        def bendable(values, structural=0):
            prefix = f'{structural}structural/' if structural else ''
            return prefix + '[' + '/'.join(map(str,values[:hard])) + ']hard/[' + '/'.join(map(str,values[hard:])) + ']soft'
        for i in range(100):
            a = bendable([rng.choice(edge) if i<50 else rng.randint(-1000000,1000000) for _ in range(hard+soft)],rng.choice([-1,0,1]))
            b = bendable([rng.choice(edge) for _ in range(hard+soft)],rng.choice([-1,0,1]))
            for op in ['parse','add','sub','neg','abs','cmp','mul','div','pow']:
                rhs = b if op in ['add','sub','cmp'] else rng.choice(['-2','-0.0','0','0.5','1','1.2','2','NaN','Infinity','-Infinity'])
                if op == 'pow': rhs = rng.choice(['0','1','2'])
                cases.append({'type':kind,'operation':op,'a':a,'b':rhs})
        for structural in [-1,0,1]:
            cases.append({'type':kind,'operation':'parse','a':bendable([0]*(hard+soft),structural),'b':''})
        cases.append({'type':kind,'operation':'parse','a':bendable(['*']*(hard+soft)),'b':''})
        valid = bendable([0]*(hard+soft))
        # Public Java adapter checks dimensions just like a fixed score definition.
        for invalid in ['', '-147', '[1]hard', valid+'junk', valid.replace('hard/','soft/'), 'badstructural/'+valid]:
            cases.append({'type':kind,'operation':'parse','a':invalid,'b':'','expect_error':True})
    return cases


def result_matches(case, java, rust):
    return java == rust and (java == 'ERROR') == case.get('expect_error', False)


def main():
    artifact = prepare_java()
    subprocess.run(['cargo', 'build', '--release', '--example', 'score_oracle', '--locked'], cwd=ROOT, check=True)
    digest = source_digest()
    cases = generate()
    payload = ''.join('\t'.join([c['type'], c['operation'], c['a'], c['b']]) + '\n' for c in cases)
    commands = {
        'java': ['java', '-cp', f'{WORK}:{JAR}', 'ScoreOracle'],
        'rust': [str(ROOT / 'target/release/examples/score_oracle')],
    }
    outputs = {name: subprocess.run(cmd, input=payload, text=True, capture_output=True, check=True).stdout.splitlines() for name, cmd in commands.items()}
    if any(len(out) != len(cases) for out in outputs.values()):
        raise RuntimeError('Oracle response count mismatch')
    failures = [{'case': case, 'java': j, 'rust': r} for case, j, r in zip(cases, outputs['java'], outputs['rust']) if not result_matches(case,j,r)]
    if source_digest() != digest: raise RuntimeError('Sources changed during comparison')
    report = {'schema_version': 1, 'generated_at': datetime.now(timezone.utc).isoformat(),
              'revision': REF['commit'], 'source_digest': digest, 'seed': 20261001,
              'total': len(cases), 'passed': len(cases)-len(failures), 'failures': failures,
              'expected_errors':sum(c.get('expect_error',False) for c in cases),
              'artifact': artifact, 'environment': platform.platform(),
              'scope': 'Original generated integer score cases: three fixed families and five Bendable shapes, including empty hard/soft groups. Public Java shape validation matches native compile-time dimensions. Unexpected ERROR results cannot pass. No additional upstream method credit; integer-exponent powers only.'}
    (ROOT / 'work/differential.json').write_text(json.dumps(report, indent=2) + '\n')
    (WORK / 'cases.json').write_text(json.dumps({'schema_version': 1, 'cases': cases}, indent=2) + '\n')
    print(f'Differential score cases: {report["passed"]}/{report["total"]} passed')
    for failure in failures[:8]:
        print(json.dumps(failure))
    return bool(failures)

if __name__ == '__main__':
    raise SystemExit(main())
