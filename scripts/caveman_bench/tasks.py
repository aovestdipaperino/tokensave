#!/usr/bin/env python3
"""Prepare a pinned task repository and independently verify trial answers."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

from run import execute

FIXTURE = Path(__file__).resolve().parent / 'fixture'
FILES = ('pricing.py', 'checkout.py', 'report.py')
PROMPTS = {
    'understanding': 'Inspect the invoice code. Write answer.json with exactly the keys '
        'entrypoint (the function called by receipt), units (the monetary unit), and '
        'rounding (the documented rounding rule). Use function names without module prefixes. '
        'Use exactly "integer cents" for units and "down to whole cents after summing" '
        'for rounding if supported by the documentation.',
    'impact': 'Find every direct caller of invoice_total and every transitive caller '
        'of discounted_total. Write answer.json with exactly direct_callers and '
        'transitive_callers, each a sorted list of function names without module prefixes. '
        'Exclude discounted_total itself.',
    'edit': 'Fix discounted_total so it rounds only after summing all discounted '
        'prices. Preserve the API and integer cents. Change only pricing.py. '
        'Check empty inputs, 0% and 100% discounts, and fractional-cent accumulation.',
}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify(task, repository):
    # The verifier and baseline live outside the editable checkout. Check the
    # complete source inventory, so a helper module cannot shadow tested imports.
    expected = set(FILES) | ({'answer.json'} if task != 'edit' else set())
    actual = {str(p.relative_to(repository)) for p in repository.rglob('*')
              if p.is_file() and '.git' not in p.relative_to(repository).parts
              and '.tokensave' not in p.relative_to(repository).parts}
    if actual != expected or any((repository / name).is_symlink() for name in expected):
        return False
    for name in FILES:
        if task == 'edit' and name == 'pricing.py':
            continue
        if digest(repository / name) != digest(FIXTURE / name):
            return False
    if task != 'edit':
        path = repository / 'answer.json'
        if path.stat().st_size > 4096:
            return False
        answer = json.loads(path.read_text())
        answers = {
            'understanding': dict(entrypoint='checkout', units='integer cents',
                                  rounding='down to whole cents after summing'),
            'impact': dict(direct_callers=['checkout', 'preview'],
                           transitive_callers=['checkout', 'invoice_total', 'preview', 'receipt']),
        }
        return answer == answers[task]
    # Run candidate code in a separate bounded process. Expected values are
    # independently computed here, outside candidate-editable source files.
    cases = [([], 25), ([1, 1], 50), ([99, 1], 10), ([101, 202], 0),
             ([101, 202], 100), ([17, 23, 41], 33)]
    cases += [([a, b], p) for a in range(1, 8) for b in range(1, 8)
              for p in (1, 25, 50, 75, 99)]
    probe = '''import json, sys
sys.path.insert(0, sys.argv[1])
from pricing import discounted_total, invoice_total
from checkout import checkout, preview
from report import receipt
cases = json.load(sys.stdin)
print(json.dumps([[f(prices, percent) for f in
    (discounted_total, invoice_total, checkout, preview, receipt)]
    for prices, percent in cases]))
'''
    code, stdout = execute([sys.executable, '-I', '-B', '-c', probe, str(repository)],
                           cwd=repository, env=dict(os.environ),
                           request=json.dumps(cases), timeout=10)
    if code or len(stdout) > 65536:
        return False
    expected = [[sum(prices) * (100 - percent) // 100] * 5 for prices, percent in cases]
    return json.loads(stdout) == expected


def prepare(output):
    output.mkdir(parents=True, exist_ok=False)
    repo = output / 'repository'
    repo.mkdir()
    for name in FILES:
        (repo / name).write_bytes((FIXTURE / name).read_bytes())
    subprocess.run(['git', 'init', '--quiet', str(repo)], check=True)
    subprocess.run(['git', 'add', '.'], cwd=repo, check=True)
    subprocess.run(['git', '-c', 'user.name=Benchmark', '-c',
                    'user.email=benchmark@example.invalid', 'commit', '--quiet',
                    '-m', 'Pinned invoice benchmark fixture'], cwd=repo, check=True)
    commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True).strip()
    config = json.loads((FIXTURE.parent / 'example.json').read_text())
    config.update(repository=str(repo), commit=commit)
    config['tasks'] = [dict(id=task, prompt=prompt, verify=[sys.executable, '-B',
        str(Path(__file__).resolve()), 'verify', task]) for task, prompt in PROMPTS.items()]
    config['verification_artifacts'] = {str(path): digest(path)
                           for path in [Path(__file__).resolve(), FIXTURE.parent / 'run.py',
                                        *(FIXTURE / n for n in FILES)]}
    (output / 'manifest.json').write_text(json.dumps(config, indent=2) + '\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    commands.add_parser('prepare').add_argument('output', type=Path)
    commands.add_parser('verify').add_argument('task', choices=PROMPTS)
    args = parser.parse_args()
    if args.command == 'prepare':
        prepare(args.output.resolve())
    else:
        try:
            passed = verify(args.task, Path.cwd())
        except (OSError, ValueError, subprocess.TimeoutExpired):
            passed = False
        sys.exit(0 if passed else 1)


if __name__ == '__main__':
    main()
