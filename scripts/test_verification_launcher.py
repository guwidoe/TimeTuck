"""Exercise the real shell launcher with a disposable check runner."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class VerificationLauncherTest(unittest.TestCase):
    def test_duplicate_paths_preserve_lookup_order_empty_entries_and_arguments(self):
        with tempfile.TemporaryDirectory(prefix='verification launcher ') as directory:
            root = Path(directory)
            scripts = root / 'scripts'
            scripts.mkdir()
            shutil.copyfile(ROOT / 'scripts/check.sh', scripts / 'check.sh')
            (scripts / 'verify_checks.py').write_text(
                'import json,os,sys\n'
                'print(json.dumps({"path":os.environ["PATH"],"args":sys.argv[1:],"cwd":os.getcwd()}))\n'
                'sys.exit(7)\n')
            entries = [str(root / 'first path'), str(Path(sys.executable).parent),
                       '/usr/bin', str(root / 'first path'), '', '/usr/bin', '',
                       str(root / 'last'), '']
            expected = os.pathsep.join(dict.fromkeys(entries))
            args = ['--force', 'argument with spaces', 'literal;$HOME', '']
            for path in [os.pathsep.join(entries), expected]:
                env = dict(os.environ, PATH=path)
                result = subprocess.run([shutil.which('bash'), str(scripts / 'check.sh'), *args],
                                        cwd='/', env=env, capture_output=True, text=True)
                self.assertEqual(result.returncode, 7, result.stderr)
                self.assertEqual(json.loads(result.stdout),
                                 dict(path=expected, args=args, cwd=str(root)))


if __name__ == '__main__':
    unittest.main()
