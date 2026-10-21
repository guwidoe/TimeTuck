#!/usr/bin/env python3
"""Prepare an isolated, audited Git mirror using an untracked redaction policy."""
import argparse
from collections import Counter
from functools import lru_cache
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys


class Redactor:
    def __init__(self, policy):
        if policy.get('schema_version') != 1:
            raise ValueError('Unsupported redaction policy.')
        self.literal = [(a.encode(), b.encode()) for a, b in policy['literal_replacements']]
        self.regex = [(re.compile(a.encode(), re.I), b.encode())
                      for a, b in policy.get('regex_replacements', [])]
        self.forbidden = [(name, re.compile(pattern.encode(), re.I))
                          for name, pattern in policy['forbidden_patterns'].items()]
        self.emails = {a.encode(): b.encode() for a, b in policy.get('email_replacements', {}).items()}
        self.scoped = [(item['contains'].encode(),
                        [(a.encode(), b.encode()) for a, b in item['literal_replacements']])
                       for item in policy.get('scoped_replacements', [])]

    def findings(self, data):
        return [name for name, pattern in self.forbidden if pattern.search(data)]

    def rewrite(self, data):
        if not self.findings(data):
            return data
        # Required author/copyright notices are not optional privacy metadata.
        for line in data.splitlines():
            if re.search(rb'copyright\s*(?:\(c\)|\xc2\xa9|[12][0-9]{3})', line, re.I) and self.findings(line):
                raise ValueError('A privacy rule overlaps a copyright notice; obtain an attribution-safe policy.')
        updated = data
        for marker, replacements in self.scoped:
            if marker in updated:
                for old, new in replacements:
                    updated = updated.replace(old, new)
        for old, new in self.literal:
            updated = updated.replace(old, new)
        for pattern, new in self.regex:
            updated = pattern.sub(new, updated)
        if self.findings(updated):
            raise ValueError('A forbidden identity remains after redaction.')
        return updated


@lru_cache(maxsize=None)
def redactor(policy_path):
    return Redactor(json.loads(Path(policy_path).read_text()))


def git(repo, *args):
    return subprocess.check_output(['git', '-C', str(repo), *args])


def audit(repo, policy_path):
    """Inspect all stored Git objects, including trees, messages, and unreachable data."""
    rules = redactor(str(policy_path))
    refs = git(repo, 'for-each-ref', '--format=%(refname)').splitlines()
    findings = [{'kind': 'ref', 'rule': rule} for ref in refs for rule in rules.findings(ref)]
    process = subprocess.Popen(['git', '-C', str(repo), 'cat-file', '--batch-all-objects', '--batch'],
                               stdout=subprocess.PIPE)
    counts = Counter()
    try:
        while header := process.stdout.readline():
            oid, kind, size = header.split()
            size = int(size)
            data = process.stdout.read(size)
            if len(data) != size or process.stdout.read(1) != b'\n':
                raise ValueError('Truncated Git object stream.')
            counts[kind.decode()] += 1
            findings.extend({'kind': kind.decode(), 'object': oid.decode(), 'rule': rule}
                            for rule in rules.findings(data))
    finally:
        process.stdout.close()
        status = process.wait()
    if status:
        raise ValueError('Git object audit failed.')
    return {'objects': dict(counts), 'refs': len(refs), 'findings': findings,
            'head': git(repo, 'rev-parse', 'HEAD').decode().strip()}


def prepare(source, destination, policy_path, filter_tool, receipts):
    source, destination, receipts = source.resolve(), destination.resolve(), receipts.resolve()
    if destination.exists() or source == destination or destination.is_relative_to(source / '.git'):
        raise ValueError('Choose a new destination outside the source Git directory.')
    if receipts.is_relative_to(destination):
        raise ValueError('Private receipts must remain outside the sanitized repository.')
    rules = redactor(str(policy_path.resolve()))
    receipts.mkdir(parents=True, exist_ok=True)
    bundle = receipts / 'original.bundle'
    if bundle.exists():
        raise ValueError('Private backup already exists; use a fresh receipt directory.')
    subprocess.run(['git', '-C', str(source), 'bundle', 'create', str(bundle), '--all'], check=True)
    # Clone the captured bundle, so a concurrent source commit cannot escape the
    # backup or make the receipt describe a different input graph.
    subprocess.run(['git', 'clone', '--mirror', str(bundle), str(destination)], check=True)
    captured = git(destination, 'rev-parse', 'HEAD').decode().strip()
    refs = git(destination, 'for-each-ref', '--format=%(objectname) %(refname)').decode().splitlines()
    names = [rules.rewrite(line.split(' ', 1)[1].encode()) for line in refs]
    if len(names) != len(set(names)):
        raise ValueError('Redaction would collide Git reference names.')
    # All private rules live outside the repository; no product identity is
    # embedded in this reusable filter or its committed test fixtures.
    module_dir = str(Path(__file__).resolve().parent)
    prelude = f'import sys; sys.path.insert(0, {module_dir!r}); from prepare_public_history import redactor; r = redactor({str(policy_path.resolve())!r}); '
    command = [sys.executable, str(filter_tool.resolve()),
        '--blob-callback', prelude + 'blob.data = r.rewrite(blob.data)',
        '--filename-callback', prelude + 'return r.rewrite(filename)',
        '--refname-callback', prelude + 'return r.rewrite(refname)',
        '--message-callback', prelude + 'return r.rewrite(message)',
        '--email-callback', prelude + 'return r.emails.get(email, email)',
        '--preserve-commit-hashes', '--replace-refs', 'delete-no-add',
        '--prune-empty', 'never', '--prune-degenerate', 'never']
    subprocess.run(command, cwd=destination, check=True)
    report = audit(destination, policy_path)
    if report['findings']:
        raise ValueError('Sanitized mirror audit found forbidden data; do not publish it.')
    # A bundle is a captured source state, never evidence of a solver execution.
    receipt = {'source_head_at_capture': captured, 'source_refs': refs,
               'source_head_after_filter': git(source, 'rev-parse', 'HEAD').decode().strip(),
               'backup_sha256': hashlib.sha256(bundle.read_bytes()).hexdigest(),
               'filter_sha256': hashlib.sha256(filter_tool.read_bytes()).hexdigest(),
               'policy_sha256': hashlib.sha256(policy_path.read_bytes()).hexdigest(),
               'audit': report}
    (receipts / 'preparation.json').write_text(json.dumps(receipt, indent=2) + '\n')
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='action', required=True)
    build = sub.add_parser('prepare', help='Back up the source and filter only a new mirror')
    build.add_argument('--source', type=Path, required=True)
    build.add_argument('--destination', type=Path, required=True)
    build.add_argument('--rules', type=Path, required=True, help='Private, untracked policy file')
    build.add_argument('--filter-tool', type=Path, required=True)
    build.add_argument('--receipts', type=Path, required=True, help='Private directory outside the new mirror')
    check = sub.add_parser('audit', help='Scan every stored Git object and reference')
    check.add_argument('--repository', type=Path, required=True)
    check.add_argument('--rules', type=Path, required=True)
    args = parser.parse_args()
    if args.action == 'prepare':
        result = prepare(args.source, args.destination, args.rules, args.filter_tool, args.receipts)['audit']
    else:
        result = audit(args.repository, args.rules)
    print(json.dumps(result))
    return bool(result['findings'])


if __name__ == '__main__':
    sys.exit(main())
