#!/usr/bin/env python3
"""Pin a prepared suite to operator-selected model, pricing and local candidates."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import re
import sys

from anthropic_driver import TOOLS, VERSION, bounded_file, driver_digest, validate_metadata
from run import execute, verify_artifacts


def configure(config, binary, tokensave_commit, caveman_source, caveman_commit,
              pricing_path, max_tokens=2048, temperature=0):
    for commit in (tokensave_commit, caveman_commit):
        if not re.fullmatch(r'[0-9a-f]{40}', commit):
            raise ValueError('pin full candidate commit SHAs')
    verify_artifacts(config)
    with binary.open('rb') as stream:
        binary_sha = hashlib.file_digest(stream, 'sha256').hexdigest()
    pricing_bytes = bounded_file(pricing_path, 65536)
    pricing = json.loads(pricing_bytes)
    metadata = dict(agent_version=VERSION, provider='anthropic', model=pricing['model'],
        model_settings=dict(max_tokens=max_tokens, temperature=temperature),
        tokensave_commit=tokensave_commit, tokensave_sha256=binary_sha,
        driver_sha256=driver_digest(), pricing_snapshot=pricing, toolset=list(TOOLS),
        experiment='voice-only', proxy_compression='off', cost_method='usage_times_pinned_prices')
    validate_metadata(metadata)
    # Verify the independent candidate exists before preparing a paid run.
    code, skill = execute(['git', 'show', caveman_commit + ':skills/caveman/SKILL.md'],
                         cwd=caveman_source, env=None, timeout=10)
    if code or len(skill.encode()) > 65536 or 'name: caveman' not in skill:
        raise ValueError('pinned independent Caveman skill unavailable')
    driver = Path(__file__).resolve().parent / 'anthropic_driver.py'
    argv = [sys.executable, '-B', str(driver), '--tokensave', str(binary)]
    result = copy.deepcopy(config)
    result['arms'] = {
        'A': dict(driver=argv, metadata=dict(metadata, caveman='absent')),
        'B': dict(driver=argv + ['--caveman-source', str(caveman_source)],
                  metadata=dict(metadata, caveman_commit=caveman_commit,
                                caveman_mode='explicit_benchmark_system_prompt',
                                caveman_skill_sha256=hashlib.sha256(skill.encode()).hexdigest())),
    }
    artifacts = result.setdefault('verification_artifacts', {})
    for path in (pricing_path, driver, driver.parent / 'transport.py', driver.parent / 'run.py'):
        artifacts[str(path)] = hashlib.sha256(path.read_bytes()).hexdigest()
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('manifest', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--tokensave', type=Path, required=True)
    parser.add_argument('--tokensave-commit', required=True)
    parser.add_argument('--caveman-source', type=Path, required=True)
    parser.add_argument('--caveman-commit', required=True)
    parser.add_argument('--pricing', type=Path, required=True)
    parser.add_argument('--max-tokens', type=int, default=2048)
    parser.add_argument('--temperature', type=float, default=0)
    args = parser.parse_args()
    result = configure(json.loads(args.manifest.read_text()), args.tokensave.resolve(),
        args.tokensave_commit, args.caveman_source.resolve(), args.caveman_commit,
        args.pricing.resolve(), args.max_tokens, args.temperature)
    with args.output.open('x') as stream:
        stream.write(json.dumps(result, indent=2) + '\n')


if __name__ == '__main__':
    main()
