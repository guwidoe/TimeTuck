"""Bounded native-goal readiness, separate from strict comparison accounting.

The manifest names the seven original completion requirements. Policy records
can resolve only an exact observed cohort; they never create Java/method passes.
Execution evidence is supplied by compat_report's checksum/source-checked loader.
"""
import hashlib
import json
from pathlib import Path
import re

from completion_progress import ALL_JOBS, BEHAVIOR_JOBS, _count, _fresh, _number, _performance, _ratio

ROOT = Path(__file__).resolve().parents[1]
BROWSER_INPUTS = frozenset(('dashboard/app.js', 'dashboard/index.html', 'dashboard/styles.css'))
PERFORMANCE_COUNTS = dict(zip(('performance', 'solver_performance', 'mixed_performance', 'heuristic_performance',
    'construction_performance', 'repair_performance', 'list_repair_performance', 'list_performance',
    'kopt_performance', 'stream_performance', 'concurrent_performance', 'sequence_performance',
    'aggregate_performance', 'pillar_performance'), (3, 10, 8, 22, 6, 4, 8, 12, 8, 8, 4, 4, 6, 6)))
# Only observations/results may improve without changing the original workload.
# Any other row field is an input declaration and requires the reviewed target.
PERFORMANCE_OUTPUT_FIELDS = frozenset(('name', 'case', 'samples', 'rust', 'java', 'java_over_rust',
    'observations', 'equal_scores', 'equal_assignments', 'rust_full', 'rust_uncached',
    'full_over_incremental', 'uncached_over_cached', 'pooled_cache_max_contributions',
    'eq_only_registry_controls', 'ordinary_stream_controls', 'owner_link_controls', 'checksum'))
REQUIREMENT_IDS = frozenset(('independent-community-solver', 'scores', 'models-moves-rollback',
                            'search', 'streams-derived-values', 'applications-services', 'dashboard'))
COHORTS = {
    'connected-range-history': ('stream_differential', 'range_collector_checks', 'accepted_native_policy'),
    'list-planning-value-metadata': ('list_differential', 'value_metadata', 'accepted_native_policy'),
    'cyclic-supplier-reference-errors': ('stream_differential', 'concurrent_checks', 'reference_unavailable'),
}
PENDING_METHODS = {
    'core/src/test/java/ai/timefold/solver/core/config/util/ConfigUtilsTest.java#ignoreSyntheticMembers:254': 'initial-jvm-scope',
    'core/src/test/java/ai/timefold/solver/core/enterprise/TimefoldSolverEnterpriseServiceTest.java#failsOnLoad:9': 'initial-jvm-scope',
    'core/src/test/java/ai/timefold/solver/core/impl/domain/common/accessor/MemberAccessorFactoryTest.java#gizmoUsesCorrectParentClassLoader:129': 'initial-jvm-scope',
    'core/src/test/java/ai/timefold/solver/core/impl/domain/solution/cloner/gizmo/GizmoSolutionClonerTest.java#debuggingDisabled:35': 'initial-jvm-scope',
    'core/src/test/java/ai/timefold/solver/core/impl/solver/DefaultSolverTest.java#solveCustomClassLoader:2183': 'initial-jvm-scope',
    'service/tools/maven-plugin/src/test/java/ai/timefold/solver/tools/maven/ConfigureMojoTest.java#testConfigureFailsForCommunityEditionBuild:489': 'initial-jvm-scope',
    'service/tools/maven-plugin/src/test/java/ai/timefold/solver/tools/maven/ConfigureMojoTest.java#testConfigureFailsForCommunityEditionBuildEvenWhenConfigurationSkipped:510': 'initial-jvm-scope',
    'service/tools/maven-plugin/src/test/java/ai/timefold/solver/tools/maven/ConfigureMojoTest.java#testConfigureSucceedsForEnterpriseEditionBuild:523': 'initial-jvm-scope',
    'core/src/test/java/ai/timefold/solver/core/impl/heuristic/selector/move/generic/list/kopt/KOptListMoveIteratorTest.java#testNonsequentialKOptOnSameEntity:361': 'native-kopt-behavior',
    'core/src/test/java/ai/timefold/solver/core/impl/heuristic/selector/move/generic/list/kopt/KOptListMoveIteratorTest.java#testNonsequentialKOptOnDifferentEntity:371': 'native-kopt-behavior',
}


def cohort_digest(value):
    """Bind all cases, outcomes, errors, counters, policies and retained controls."""
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':'),
                                     allow_nan=False).encode()).hexdigest()


def workload_declaration(row):
    """Original inputs only; do not freeze timings, results or callback counts."""
    if not isinstance(row, dict) or not isinstance(row.get('name'), str) or not row['name']:
        raise ValueError('Every original workload needs its exact name.')
    return {'name': row['name'], 'case_sha256': cohort_digest(row['case']) if 'case' in row else None,
            'inputs': {key: value for key, value in row.items() if key not in PERFORMANCE_OUTPUT_FIELDS}}


def _path(root, value):
    if not isinstance(value, str) or not value or Path(value).is_absolute():
        raise ValueError('Evidence paths must be repository-relative.')
    path = (root / value).resolve()
    if not path.is_relative_to(root.resolve()):
        raise ValueError('Evidence path escapes repository.')
    return path


def _bound_file(root, evidence):
    if (not isinstance(evidence, dict) or set(evidence) != {'path', 'sha256'} or
            not isinstance(evidence['sha256'], str) or
            not re.fullmatch('[0-9a-f]{64}', evidence['sha256'])):
        raise ValueError('Invalid checksum-bound evidence.')
    path = _path(root, evidence['path'])
    raw = path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != evidence['sha256']:
        raise ValueError(f'Evidence checksum mismatch: {evidence["path"]}')
    return raw


def load_manifest(root=ROOT):
    return json.loads((root / 'compat/native-targets.json').read_text())


def _validate(manifest):
    if (not isinstance(manifest, dict) or set(manifest) !=
            {'schema_version', 'target_set', 'scope', 'requirements', 'boundaries', 'method_boundaries', 'performance_targets'} or
            manifest['schema_version'] != 1 or manifest['target_set'] != 'original-native-goal-v1'):
        raise ValueError('Unknown native target schema/set; no scope reduction is inferred.')
    targets = manifest['performance_targets']
    if not isinstance(targets, dict) or set(targets) != set(PERFORMANCE_COUNTS):
        raise ValueError('All fourteen original performance target families are required.')
    for name, count in PERFORMANCE_COUNTS.items():
        target = targets[name]
        if (not isinstance(target, dict) or set(target) != {'config', 'workloads'} or
                not isinstance(target['config'], dict) or not isinstance(target['workloads'], list) or
                len(target['workloads']) != count):
            raise ValueError(f'Original workload target count/config is required: {name}.')
        names = set()
        for row in target['workloads']:
            if (not isinstance(row, dict) or set(row) != {'name', 'case_sha256', 'inputs'} or
                    not isinstance(row['name'], str) or not row['name'] or row['name'] in names or
                    not isinstance(row['inputs'], dict) or
                    (row['case_sha256'] is not None and (not isinstance(row['case_sha256'], str) or
                     not re.fullmatch('[0-9a-f]{64}', row['case_sha256'])))):
                raise ValueError(f'Invalid/duplicate original workload declaration: {name}.')
            names.add(row['name'])
    requirements = manifest['requirements']
    if (not isinstance(requirements, list) or len(requirements) != len(REQUIREMENT_IDS) or
            {r.get('id') for r in requirements if isinstance(r, dict)} != REQUIREMENT_IDS):
        raise ValueError('The complete fixed original native requirement set is required.')
    for item in requirements:
        if set(item) != {'id', 'label', 'contract', 'tests', 'files', 'jobs', 'limits', 'browser_evidence'}:
            raise ValueError('Unknown requirement fields.')
        if any(not isinstance(item[key], str) or not item[key] for key in ('label', 'contract', 'limits')):
            raise ValueError('Requirement contract and limits are required.')
        if (not isinstance(item['tests'], list) or not isinstance(item['files'], list) or
                not isinstance(item['jobs'], list) or not set(item['jobs']) <= set(ALL_JOBS)):
            raise ValueError('Invalid requirement evidence.')
        if item['id'] == 'dashboard':
            if not isinstance(item['browser_evidence'], dict):
                raise ValueError('Dashboard rendering evidence is required.')
        elif not item['tests'] or item['browser_evidence'] is not None:
            raise ValueError('Named native executable proof is required.')
        identities = []
        for test in item['tests']:
            if (not isinstance(test, dict) or set(test) != {'path', 'name'} or
                    any(not isinstance(test[k], str) or not test[k] for k in test)):
                raise ValueError('Exact native test path/name is required.')
            identities.append((test['path'], test['name']))
        if len(set(identities)) != len(identities):
            raise ValueError('Duplicate native proof identity.')
    boundaries = manifest['boundaries']
    if (not isinstance(boundaries, list) or len(boundaries) != len(COHORTS) or
            {b.get('id') for b in boundaries if isinstance(b, dict)} != set(COHORTS)):
        raise ValueError('All exact native policy/reference boundaries are required.')
    for boundary in boundaries:
        if set(boundary) != {'id', 'job', 'check', 'disposition', 'contract', 'reason',
                             'authority', 'evidence', 'cohort_sha256', 'requirements'}:
            raise ValueError('Unknown boundary fields; blanket acceptance is forbidden.')
        if (boundary['job'], boundary['check'], boundary['disposition']) != COHORTS[boundary['id']]:
            raise ValueError('Boundary cannot resolve another cohort or disposition.')
        if (not boundary['requirements'] or not set(boundary['requirements']) <= REQUIREMENT_IDS or
                any(not isinstance(boundary[k], str) or not boundary[k]
                    for k in ('contract', 'reason', 'authority', 'cohort_sha256'))):
            raise ValueError('Boundary authority, exact cohort and native proof are required.')
        if (boundary['disposition'] == 'accepted_native_policy' and
                boundary['authority'] != 'Explicit user reply in this project chat'):
            raise ValueError('Explicit user policy authority is required.')
    mappings = manifest['method_boundaries']
    if (not isinstance(mappings, list) or len(mappings) != len(PENDING_METHODS) or
            {m.get('method_id'): m.get('disposition') for m in mappings if isinstance(m, dict)} != PENDING_METHODS):
        raise ValueError('Only the ten exact original pending method boundaries are recognized.')
    for mapping in mappings:
        if (set(mapping) != {'method_id', 'disposition', 'contract', 'requirement', 'limits'} or
                mapping['requirement'] not in REQUIREMENT_IDS or
                any(not isinstance(mapping[k], str) or not mapping[k] for k in ('contract', 'limits'))):
            raise ValueError('Invalid original-scope method mapping.')
    return manifest


def model_digest(manifest=None, root=ROOT):
    """Presentation-policy identity only; never a Rust or measurement input key."""
    manifest = load_manifest(root) if manifest is None else manifest
    chunks = [Path(__file__).read_bytes(), json.dumps(manifest, sort_keys=True, allow_nan=False).encode()]
    # Hash actual policy/receipt bytes as well as their reviewed expected hashes.
    # Missing evidence stays deterministically identifiable, and never passes.
    references = [b.get('evidence') for b in manifest.get('boundaries', []) if isinstance(b, dict)]
    for item in manifest.get('requirements', []):
        if isinstance(item, dict):
            references += item.get('files', []) + [item.get('browser_evidence')]
    for ref in references:
        if isinstance(ref, dict) and isinstance(ref.get('path'), str):
            try:
                chunks.append(_path(root, ref['path']).read_bytes())
            except (OSError, ValueError):
                chunks.append(b'missing:' + ref['path'].encode())
    # A receipt remains byte-identical after a UI edit. Include its actual current
    # inputs so live freshness changes immediately, before report regeneration.
    for item in manifest.get('requirements', []):
        evidence = item.get('browser_evidence') if isinstance(item, dict) else None
        if not isinstance(evidence, dict) or not isinstance(evidence.get('path'), str):
            continue
        try:
            proof = json.loads(_path(root, evidence['path']).read_bytes())
            inputs = proof.get('input_sha256') if isinstance(proof, dict) else None
            if not isinstance(inputs, dict):
                raise ValueError('Browser input bindings are missing.')
        except (OSError, ValueError, TypeError):
            chunks.append(b'invalid-browser-inputs:' + evidence['path'].encode())
            continue
        for path in sorted(inputs):
            chunks.append(b'browser-input:' + path.encode())
            try:
                chunks.append(b'present:' + _path(root, path).read_bytes())
            except (OSError, ValueError):
                chunks.append(b'missing-or-invalid:' + path.encode())
    return hashlib.sha256(b'\0'.join(chunks)).hexdigest()


def named_test_evidence(results, native_digest, full_gate=False):
    """Only call after trusted run parsing verifies log checksum/source binding."""
    return {'source_digest': native_digest, 'full_gate': full_gate,
            'tests': [{'path': path, 'name': name, 'status': status}
                      for (path, name), status in sorted(results.items())]}


def _browser(root, evidence):
    proof = json.loads(_bound_file(root, evidence))
    if proof.get('passed') is not True or proof.get('errors') != []:
        return False
    hashes = proof.get('input_sha256')
    if not isinstance(hashes, dict) or not BROWSER_INPUTS <= hashes.keys():
        return False
    if any(hashlib.sha256(_path(root, path).read_bytes()).hexdigest() != digest
           for path, digest in hashes.items()):
        return False
    views = proof.get('proof', [])
    return (isinstance(views, list) and
            all(any(isinstance(view, dict) and view.get('label') == label and
                        type(view.get('width')) is int and view['width'] > 0 and
                        view.get('overflow') is False for view in views)
                for label in ('desktop', 'mobile')))


def build_native_readiness(report, strict_completion, manifest=None, root=ROOT):
    """Fail closed without changing any strict result or raw report observation."""
    result = {'schema_version': 1, 'target_set': 'original-native-goal-v1', 'native_ready': False,
              'ready': False, 'model_digest': None, 'requirements': [], 'boundaries': [],
              'checklist': [], 'blockers': [], 'method_boundaries': [],
              'scope': 'Bounded native capability and measured original-workload readiness; not whole-Timefold equivalence.',
              'accounting': 'Strict adapted-method/Java-comparison counters, errors, unsupported observations and readiness are unchanged.'}
    try:
        manifest = _validate(load_manifest(root) if manifest is None else manifest)
        result['model_digest'] = model_digest(manifest, root)
        result['scope'] = manifest['scope']
    except (OSError, ValueError, TypeError, KeyError, AttributeError) as error:
        result['blockers'].append(f'Native target evidence is invalid or missing: {error}')
        return result
    report = report if isinstance(report, dict) else {}
    verification = report.get('verification', {})
    verification = verification if isinstance(verification, dict) else {}
    proof = verification.get('native_tests', {})
    proof = proof if isinstance(proof, dict) else {}
    digest = report.get('native_source_digest')
    gate = (verification.get('successful') is True and proof.get('full_gate') is True and
            isinstance(digest, str) and bool(re.fullmatch('[0-9a-f]{64}', digest)) and
            proof.get('source_digest') == digest)
    passed = set()
    states = {}
    for test in proof.get('tests', []) if isinstance(proof.get('tests'), list) else []:
        if isinstance(test, dict) and all(isinstance(test.get(key), str) for key in ('path', 'name', 'status')):
            identity = (test['path'], test['name'])
            # Duplicate or contradictory proof is not a trusted single outcome.
            states[identity] = test['status'] if identity not in states else 'unverified'
    passed.update(identity for identity, status in states.items() if status == 'passed')
    for item in manifest['requirements']:
        missing = [test for test in item['tests'] if (test['path'], test['name']) not in passed]
        errors = []
        try:
            for evidence in item['files']:
                _bound_file(root, evidence)
            if item['browser_evidence'] is not None and not _browser(root, item['browser_evidence']):
                errors.append('Current desktop/mobile rendering proof is missing or stale.')
        except (OSError, ValueError, TypeError, KeyError, AttributeError) as error:
            errors.append(str(error))
        current = all(_fresh(report.get(job)) == 'current' for job in item['jobs'])
        met = gate and not missing and not errors and current
        result['requirements'].append({**item, 'status': 'met' if met else 'unknown',
                                       'source_digest': digest, 'missing_tests': missing, 'evidence_errors': errors})
    requirements = {item['id']: item['status'] == 'met' for item in result['requirements']}
    resolved = set()
    for boundary in manifest['boundaries']:
        job = report.get(boundary['job'], {})
        cohort = job.get(boundary['check']) if isinstance(job, dict) else None
        errors = []
        try:
            record = json.loads(_bound_file(root, boundary['evidence']))
            if not isinstance(cohort, dict) or cohort_digest(cohort) != boundary['cohort_sha256']:
                errors.append('Observed cohort differs from the exact reviewed policy/reference evidence.')
            if boundary['disposition'] == 'accepted_native_policy':
                decision = record.get('completion_policy_decision', {})
                answer = {'connected-range-history': 'Keep consistent native behavior',
                          'list-planning-value-metadata': 'Keep stable native capture'}[boundary['id']]
                if (not isinstance(decision, dict) or decision.get('authority') != boundary['authority'] or
                        decision.get('answer') != answer or not decision.get('target')):
                    errors.append('Explicit matching user policy authority is missing.')
            elif boundary['authority'] != 'Observed unavailable Java reference; independent native cycle/recovery proof':
                errors.append('Unavailable-reference authority is invalid.')
            elif (not isinstance(cohort, dict) or
                  cohort_digest(record.get('concurrent_checks')) != boundary['cohort_sha256'] or
                  cohort.get('divergent_cases') != 0 or cohort.get('unsupported_cases', 0) != 0 or
                  not _count(cohort.get('java_error_cases')) or
                  not _count(cohort.get('passed')) or
                  cohort['passed'] + cohort['java_error_cases'] != cohort.get('total')):
                errors.append('Reference-unavailable disposition covers only the exact actual Java errors.')
            if (not isinstance(cohort, dict) or not _count(cohort.get('total')) or cohort['total'] == 0 or
                    cohort.get('native_reference_passed') != cohort['total']):
                errors.append('Complete independent native cohort verification is missing.')
        except (OSError, ValueError, TypeError, KeyError, AttributeError) as error:
            errors.append(str(error))
        if not all(requirements.get(name) for name in boundary['requirements']):
            errors.append('Source-bound native requirement proof is incomplete.')
        met = not errors and _fresh(job) == 'current'
        if met:
            resolved.add((boundary['job'], boundary['check']))
        counts = {key: cohort[key] for key in ('total', 'passed', 'native_reference_passed', 'java_error_cases',
                                             'unsupported_cases', 'divergent_cases', 'order_differences')
                  if isinstance(cohort, dict) and key in cohort}
        result['boundaries'].append({**boundary, 'status': 'resolved' if met else 'unresolved',
                                     'counts': counts, 'blockers': errors})
    behavior_ok = True
    for name in BEHAVIOR_JOBS:
        job = report.get(name, {})
        checks = [('top-level', job)] + ([(key, value) for key, value in job.items()
                  if isinstance(value, dict) and ('total' in value or 'passed' in value)] if isinstance(job, dict) else [])
        for check, value in checks:
            if (_ratio(value) != 1 or any(value.get(key, 0) != 0 for key in ('java_error_cases', 'unsupported_cases', 'divergent_cases'))):
                if (name, check) not in resolved:
                    behavior_ok = False
                    result['blockers'].append(f'Unresolved behavior evidence: {name}.{check}.')
    methods = report.get('methods')
    methods_ok = isinstance(methods, list) and bool(methods)
    lookup = {m.get('id'): m for m in methods if isinstance(m, dict)} if methods_ok else {}
    for mapping in manifest['method_boundaries']:
        method = lookup.get(mapping['method_id'])
        met = (isinstance(method, dict) and method.get('status') in ('pending', 'passed') and
               requirements.get(mapping['requirement'], False))
        result['method_boundaries'].append({**mapping, 'status': 'resolved' if met else 'unresolved',
                                           'original_status': method.get('status') if isinstance(method, dict) else 'unknown'})
        methods_ok = methods_ok and met
    if any(m.get('status') == 'pending' and m.get('id') not in PENDING_METHODS for m in lookup.values()):
        methods_ok = False
    summary = report.get('summary', {})
    clean = isinstance(summary, dict) and all(_count(summary.get(key)) and summary[key] == 0 for key in ('failed', 'unverified'))
    current = sum(_fresh(report.get(name)) == 'current' for name in ALL_JOBS)
    # Reuse exactly the strict timing/quality evaluator and resolved target.
    target = strict_completion.get('target', {}).get('java_time_multiplier') if isinstance(strict_completion, dict) else None
    performance = _performance(report, target) if _number(target) and target > 0 else ([], [])
    performance_ok = bool(performance[0]) and all(item['status'] == 'met' for item in performance[0])
    result['performance'] = performance[0]
    workload_targets_ok = True
    for name, target_spec in manifest['performance_targets'].items():
        job = report.get(name)
        try:
            rows = job.get('workloads') if isinstance(job, dict) else None
            matches = (isinstance(rows, list) and len(rows) == PERFORMANCE_COUNTS[name] and
                       isinstance(job.get('config'), dict) and
                       cohort_digest(job['config']) == cohort_digest(target_spec['config']) and
                       cohort_digest([workload_declaration(row) for row in rows]) ==
                       cohort_digest(target_spec['workloads']))
        except (ValueError, TypeError, KeyError):
            matches = False
        if not matches:
            workload_targets_ok = False
            result['blockers'].append(f'Original workload universe/configuration is missing or changed: {name}.')
    performance_ok = performance_ok and workload_targets_ok
    checks = [('current-full-native-verification', gate, 'Current source-bound successful full native test gate.'),
              ('raw-status-clear', clean, 'Original failed/unverified method counts must be zero.'),
              ('current-measurements', current == len(ALL_JOBS), f'{current}/{len(ALL_JOBS)} original measurement jobs current.'),
              ('original-performance-target', performance_ok, 'Every original workload meets the strict equal-or-better quality/time target.'),
              ('native-requirements', all(requirements.values()), 'All seven original native requirements have executable evidence.'),
              ('behavior-boundaries', behavior_ok and len(resolved) == len(COHORTS), 'Every observed behavior boundary is resolved for its exact cohort.'),
              ('original-method-boundaries', methods_ok, 'Exact pending JVM/private-array mappings; no inventory credit.')]
    result['checklist'] = [{'id': key, 'status': 'met' if met else 'not_met', 'detail': detail} for key, met, detail in checks]
    result['blockers'] += [detail for _, met, detail in checks if not met]
    for item in result['requirements']:
        if item['status'] != 'met':
            result['blockers'].append(f'Native requirement not demonstrated: {item["label"]}.')
    result['native_ready'] = result['ready'] = all(met for _, met, _ in checks)
    return result
