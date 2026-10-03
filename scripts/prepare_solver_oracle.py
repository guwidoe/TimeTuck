#!/usr/bin/env python3
"""Resolve the pinned public Community artifact and its runtime dependencies."""
import hashlib
import os
import subprocess
import urllib.request
import zipfile
from pathlib import Path
from differential_scores import ROOT, WORK, REF

MAVEN_VERSION = '3.9.16'


def prepare_solver_java():
    # Isolate build downloads and outputs in ignored work/, not the reference tree.
    work = WORK / 'solver'
    work.mkdir(parents=True, exist_ok=True)
    maven_home = work / f'apache-maven-{MAVEN_VERSION}'
    maven = maven_home / 'bin/mvn'
    if not maven.exists():
        url = f'https://repo.maven.apache.org/maven2/org/apache/maven/apache-maven/{MAVEN_VERSION}/apache-maven-{MAVEN_VERSION}-bin.zip'
        archive = work / 'maven.zip'
        urllib.request.urlretrieve(url, archive)
        expected = urllib.request.urlopen(url + '.sha512').read().decode().split()[0]
        if hashlib.sha512(archive.read_bytes()).hexdigest() != expected:
            raise RuntimeError('Maven distribution checksum mismatch')
        with zipfile.ZipFile(archive) as z:
            for name in z.namelist():
                if not (work / name).resolve().is_relative_to(work.resolve()):
                    raise RuntimeError('Unsafe Maven archive path')
            z.extractall(work)
        maven.chmod(0o755)
    dependencies = work / 'dependencies'
    # Always resolve the pinned POM, recording every artifact used below.
    subprocess.run([str(maven), '-q', '-f', str(ROOT / 'compat/oracle/pom.xml'),
                    'org.apache.maven.plugins:maven-dependency-plugin:3.8.1:copy-dependencies',
                    '-DincludeScope=runtime', f'-DoutputDirectory={dependencies}'], check=True, cwd=ROOT)
    jars = sorted(dependencies.glob('*.jar'))
    core = [p for p in jars if p.name.startswith('timefold-solver-core-')]
    if [p.name for p in core] != [f'timefold-solver-core-{REF["tag"][1:]}.jar']:
        raise RuntimeError('Expected exactly the pinned Community core artifact')
    classpath = os.pathsep.join([str(work), *(str(p) for p in jars)])
    subprocess.run(['javac', '-cp', classpath, '-d', str(work), str(ROOT / 'compat/oracle/SolverOracle.java'), str(ROOT / 'compat/oracle/RangeSolverOracle.java'), str(ROOT / 'compat/oracle/SolverBenchmark.java'), str(ROOT / 'compat/oracle/HeuristicBenchmark.java'), str(ROOT / 'compat/oracle/TerminationOracle.java')], check=True)
    artifacts = [{'name': p.name, 'sha256': hashlib.sha256(p.read_bytes()).hexdigest()} for p in jars]
    return classpath, artifacts


if __name__ == '__main__':
    print(prepare_solver_java()[0])
