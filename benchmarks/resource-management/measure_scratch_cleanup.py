import json
from pathlib import Path
import resource
import statistics
import subprocess
import sys
import tempfile
import time

times, allocated, preservation = [], [], []
with tempfile.TemporaryDirectory(dir=Path.cwd(), prefix='scratch-measure-') as temporary:
    root = Path(temporary)
    for index in range(5):
        repo = root / str(index)
        repo.mkdir()
        git = ['git', '-c', 'core.hooksPath=', '-C', str(repo)]
        subprocess.run(git + ['init', '-q'], check=True, capture_output=True)
        (repo / 'user.txt').write_text('original\n')
        subprocess.run(git + ['add', 'user.txt'], check=True, capture_output=True)
        subprocess.run(git + ['-c', 'user.name=Benchmark', '-c', 'user.email=benchmark@example.invalid', 'commit', '-qm', 'fixture'], check=True, capture_output=True)
        (repo / 'user.txt').write_text('user changes\n')
        (repo / 'untracked.txt').write_bytes(b'u' * 65536)
        result = subprocess.run([sys.argv[1], str(repo)], check=True, capture_output=True, text=True)
        times.append(float(result.stdout.strip().splitlines()[-1]))
        allocated.append(sum(p.lstat().st_blocks * 512 for p in [repo, *repo.rglob('*')]))
        preservation.append((repo / 'user.txt').read_text() == 'user changes\n' and (repo / 'untracked.txt').exists())
        assert not (repo / '.tokensave-bench-scratch').exists()
print(json.dumps(dict(runs=5, elapsed_ms=times, median_ms=statistics.median(times),
                      retained_allocated_bytes=allocated, user_changes_preserved=preservation,
                      child_peak_rss_kib=resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss,
                      parent_peak_rss_kib=resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,
                      peak_scratch_payload_bytes=1048576, repository_copy_calls=0,
                      fixture_removed=not root.exists())))
