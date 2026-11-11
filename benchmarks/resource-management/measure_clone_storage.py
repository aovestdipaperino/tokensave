import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import resource
import statistics
import subprocess
import sys
import tempfile
import time

spec = importlib.util.spec_from_file_location('bench', sys.argv[1])
bench = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bench)
# Equivalent local clone workload; directory selection itself is tested separately.
with tempfile.TemporaryDirectory(dir=Path.cwd(), prefix='resource-measure-') as temporary:
    root = Path(temporary)
    source = root / 'source'
    source.mkdir()
    (source / 'sample.py').write_bytes(b'# benchmark fixture\n' * 55000)
    git = ['git', '-c', 'core.hooksPath=', '-C', str(source)]
    subprocess.run(git + ['init', '-q'], check=True)
    subprocess.run(git + ['add', '.'], check=True)
    subprocess.run(git + ['-c', 'user.name=Benchmark', '-c', 'user.email=benchmark@example.invalid', 'commit', '-qm', 'fixture'], check=True)
    def allocated(path):
        return sum(p.lstat().st_blocks * 512 for p in [path, *path.rglob('*')])
    times, sizes, reused = [], [], []
    for index in range(5):
        cache = root / f'cache-{index}'
        os.environ['TOKENSAVE_BENCH_REPOS_DIR'] = str(cache)
        bench.CLONE_DIR = cache  # old module's hard-coded root, isolated for measurement
        started = time.perf_counter()
        with contextlib.redirect_stderr(io.StringIO()):
            dest = bench.clone_repo('fixture', source.as_uri(), False)
        times.append((time.perf_counter() - started) * 1000)
        sizes.append(allocated(cache))
        started = time.perf_counter()
        with contextlib.redirect_stderr(io.StringIO()):
            for _ in range(100):
                bench.clone_repo('fixture', source.as_uri(), True)
        reused.append((time.perf_counter() - started) * 1000)
    result = dict(clone_ms=times, median_clone_ms=statistics.median(times),
                  reuse_100_ms=reused, median_reuse_100_ms=statistics.median(reused),
                  cache_allocated_bytes=sizes, total_allocated_bytes=allocated(root),
                  parent_peak_rss_kib=resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,
                  child_peak_rss_kib=resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss,
                  copies=0, build_artifacts_bytes=0,
                  default_tmp_new_bytes=0)
assert not root.exists()
result['measurement_fixture_removed'] = True
print(json.dumps(result))
