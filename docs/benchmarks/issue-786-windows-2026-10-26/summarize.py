import collections,csv,html,json,math,pathlib,random,statistics
R=pathlib.Path(__file__).resolve().parent
raw=list(csv.DictReader((R/'benchmark-results.csv').open(encoding='utf-8')))
groups=collections.defaultdict(dict)
for row in raw:groups[row['workload']].setdefault(row['binary'],[]).append(row)
rng=random.Random(786)
def quant(a,q):return sorted(a)[max(0,math.ceil(len(a)*q)-1)]
summ=[]
for name,g in groups.items():
    if set(g)!= {'stable','beta'}:continue
    a=[float(x['elapsed_ms']) for x in g['stable']];b=[float(x['elapsed_ms']) for x in g['beta']]
    if min(len(a),len(b))<2:continue
    sa=statistics.median(a);sb=statistics.median(b)
    n=min(len(a),len(b));d=[]
    for _ in range(3000):
        idx=[rng.randrange(n) for _ in range(n)]
        d.append(statistics.median([a[i] for i in idx])-statistics.median([b[i] for i in idx]))
    lo=quant(d,.025);hi=quant(d,.975)
    summ.append(dict(workload=name,n=n,unit='bytes' if name=='database_bytes' else 'ms',stable_p50=sa,beta_p50=sb,delta_beta_minus_stable=sb-sa,improvement_percent=(sa-sb)/sa*100 if sa else 0,improvement_ci95_low=lo,improvement_ci95_high=hi,stable_p95=quant(a,.95),beta_p95=quant(b,.95),stable_p99=quant(a,.99),beta_p99=quant(b,.99),stable_cpu_seconds=statistics.mean(float(x['cpu_seconds']) for x in g['stable']),beta_cpu_seconds=statistics.mean(float(x['cpu_seconds']) for x in g['beta']),stable_cpu_percent=statistics.mean(float(x['cpu_percent_one_core']) for x in g['stable']),beta_cpu_percent=statistics.mean(float(x['cpu_percent_one_core']) for x in g['beta']),stable_max_rss=max(int(x['peak_rss_bytes']) for x in g['stable']),beta_max_rss=max(int(x['peak_rss_bytes']) for x in g['beta'])))
with (R/'benchmark-summary.csv').open('w',newline='',encoding='utf-8') as f:
    w=csv.DictWriter(f,fieldnames=summ[0].keys());w.writeheader();w.writerows(summ)
checks=json.loads((R/'correctness.json').read_text());env=json.loads((R/'environment.json').read_text(encoding='utf-8-sig'));bins=json.loads((R/'binaries.json').read_text());cfg=json.loads((R/'benchmark-config.json').read_text())
extended=json.loads((R/'extended-results.json').read_text()) if (R/'extended-results.json').exists() else {}
def table(items):
    lines=['| Workload | n / binary | Unit | Stable p50 | Beta p50 | Beta − Stable | Improvement | 95% CI of Stable − Beta |','|---|---:|---|---:|---:|---:|---:|---:|']
    for x in items:lines.append(f"| {x['workload']} | {x['n']} | {x['unit']} | {x['stable_p50']:.3f} | {x['beta_p50']:.3f} | {x['delta_beta_minus_stable']:+.3f} | {x['improvement_percent']:+.1f}% | [{x['improvement_ci95_low']:.3f}, {x['improvement_ci95_high']:.3f}] |")
    return '\n'.join(lines)
fail=[x for x in checks if not x['passed']]
reg=[x for x in summ if x['unit']=='ms' and x['improvement_ci95_high']<0 and x['improvement_percent']<-5]
sus=[x for x in summ if x['unit']=='ms' and x['improvement_percent']<-5 and x not in reg]
text=f'''# TokenSave Beta — Issue #786 Windows Validation Report

**Environment**

- OS: {env['os']['Caption']}, version {env['os']['Version']}, build {env['os']['BuildNumber']}.
- CPU: {env['cpu']['Name']}; {env['cpu']['NumberOfLogicalProcessors']} logical processors.
- Installed physical memory reported by Windows: {env['memory']['TotalPhysicalMemory']} bytes.
- Storage: {env['disks']['FriendlyName']}, {env['disks']['MediaType']}.
- Power profile: {env['power']}.
- Stable: `{bins['stable']['version']}`; SHA-256 `{bins['stable']['sha256']}`.
- Beta: `{bins['beta']['version']}`; SHA-256 `{bins['beta']['sha256']}`.
- Workload repository: local `e-shock_b-challenger`; commit `{cfg['commit']}`. Only committed C/C++ headers/sources and Python files were exported; manifest includes source hashes and relocated paths. Added synthetic Python fixtures exercise duplicate method names and inheritance.
- Binary source commits: **unverified**. Filenames identify the supplied channels; identical version strings cannot establish that Beta contains the agreed integration tip.
- Benchmark start: `{bins['date']}` (local timestamp). Machine metadata: `environment.json`; binary identity: `binaries.json`.
- Methodology: serial execution, alternating Stable/Beta order per repetition, {cfg['index_repetitions']} indexing repetitions, {cfg['cli_repetitions']} measured CLI repetitions and {cfg['mcp_repetitions']} measured persistent MCP repetitions per query; {cfg['warmups']} warmup rounds per query excluded. CLI includes process startup and database open. MCP includes local JSON-RPC transport and excludes process initialization. Both servers stay open during the MCP sequence. CPU profile is `{cfg['cpu_usage']}`. OS caches are not flushed. No machine isolation or thermal control was imposed.

## 1. Executive summary

This is an external Windows binary comparison, not confirmation of the upstream Rust/SQL test suite. The measurements below describe this exported firmware/synthetic corpus only. Overall superiority and functional equivalence are **not established**. Results are workload-dependent; uncertainty intervals that cross zero do not establish an improvement or regression.

There are {len(checks)} automated checks: {len(checks)-len(fail)} successful and {len(fail)} failed strict comparisons/checks. A failed exact-output comparison is an investigation item, not automatically a semantic defect. Short serial MCP operation does not establish sustained production stability. Memory figures are observations, not proof of a leak or absence of one.

Recommendation: **NEEDS ADDITIONAL VALIDATION** before promotion. Keep #786 open pending provenance, output-difference review, production concurrency/cancellation and the upstream completion requirements.

## 2. Performance comparison

Positive improvement means Beta is faster/smaller. Absolute delta is Beta minus Stable. Confidence intervals are paired percentile bootstrap intervals for the difference of medians (3,000 resamples, deterministic seed 786). They are exploratory, unadjusted for multiple comparisons, and assume rounds are exchangeable; system drift can violate that assumption. Indexing intervals with only five repetitions are weak evidence.

{table(summ)}

### Latency percentiles

Nearest-rank sample percentiles, not established production tails. CLI n=15 and indexing n=5 are far too small for useful p99 inference; even MCP n=100 makes p99 depend on the top few samples.

| Workload | Stable p95 ms | Beta p95 ms | Stable p99 ms | Beta p99 ms |
|---|---:|---:|---:|---:|
'''
for x in summ:
    if x['unit']=='ms':text+=f"| {x['workload']} | {x['stable_p95']:.3f} | {x['beta_p95']:.3f} | {x['stable_p99']:.3f} | {x['beta_p99']:.3f} |\n"
text+='\n### CPU, memory and persistent throughput\n\nCPU utilization is process CPU time / measured wall time, with 100% representing one logical processor; divide by eight for machine capacity. CLI process CPU and RSS are polled every approximately 5 ms, so short-lived peaks/final CPU ticks can be missed. MCP RSS is sampled at request boundaries and is **not true peak RAM**. CPU tick quantization makes individual short MCP CPU measurements unreliable. Child processes are excluded. Database size counts `.db` files, excluding WAL/SHM.\n\n| Workload | Stable mean CPU s | Beta mean CPU s | Stable CPU % | Beta CPU % | Stable max observed RSS bytes | Beta max observed RSS bytes |\n|---|---:|---:|---:|---:|---:|---:|\n'
for x in summ:
    if x['unit']=='ms':text+=f"| {x['workload']} | {x['stable_cpu_seconds']:.6f} | {x['beta_cpu_seconds']:.6f} | {x['stable_cpu_percent']:.2f} | {x['beta_cpu_percent']:.2f} | {x['stable_max_rss']} | {x['beta_max_rss']} |\n"
for label in ('stable','beta'):
    rr=[r for r in raw if r['binary']==label and r['workload'].startswith('mcp_')]
    elapsed=sum(float(r['elapsed_ms']) for r in rr)/1000
    text+=f"\n{label.title()} persistent MCP: {len(rr)} measured requests, {elapsed:.3f} s summed request wall time, {len(rr)/elapsed:.2f} requests/s (serial active-service rate, excludes harness gaps and other binary's requests; not concurrent end-to-end throughput).\n"
text+='\n## 3. Optimization-by-optimization validation\n\nAll measurements are external CLI/MCP behavior. Attribution to an individual PR is inconclusive because these are combined binaries with unverified source provenance. Rows below refer directly to the full measurements in Section 2 and the CSV, including Stable/Beta medians, intervals and repetitions.\n'
pairs=[('#775 / #776','initial_index, forced_index, incremental_index, deletion, database_bytes','File addition/deletion search checks, CLI verify and SQLite integrity; no rollback/cancellation or dispatch-index SQL-plan assertion.'),('#777 / #779','cli_complexity, cli_complexity_scoped, mcp_complexity, mcp_complexity_scoped','Exact ranked-output comparisons; project/time savings index is not directly exercised.'),('#778 / #781','cli_body, cli_ambiguous, cli_ambiguous_calls, mcp_body, mcp_ambiguous, mcp_ambiguous_calls','Unique body and duplicate method candidates; no direct executable-body SQL or candidate-membership oracle.'),('#782 / #783','cli_inheritance, cli_inheritance_scoped, mcp_inheritance, mcp_inheritance_scoped','Synthetic same-file inheritance; out-of-scope ancestors and deep/cyclic inheritance are not independently asserted.'),('#784 / #785','cli_hotspots, mcp_hotspots','Connectivity ranking surface exercised; individual Q0118/Q0130 SQL paths and hydration equivalence not proven.'),('#751 / #787','cli_fts_common, cli_fts_rare, cli_fts_empty, mcp_fts_common, mcp_fts_rare, mcp_fts_empty','Common, rare and absent-name search; CLI search is not a direct isolated FTS SQL benchmark. All-matching/adversarial production-scale corpus not covered.')]
for title,names,limit in pairs:
    selected=[x for x in summ if x['workload'] in names.split(', ')]
    text+=f'\n### {title}\n\n{table(selected)}\n\nTests/correctness and limitations: {limit} Exact-output results are in `correctness.json`; failures are listed below. Improvements/regressions are the signed percentages and confidence intervals above. **Result: INCONCLUSIVE** for complete implementation validation.\n'
text+='\n## 4. Correctness and regression testing\n\n'
text+=f'Executed {len(checks)} checks, with {len(checks)-len(fail)} successful and {len(fail)} unsuccessful. Every individual result and detail is in `correctness.json`. Query success/error status is additionally recorded per measured request in `benchmark-results.csv`.\n\n'
text+='- Exact comparisons preserve result order. Only JSON-RPC IDs, timing `_meta`, and known project roots are normalized. Timing/warning/counter text may still cause differences. No failed equality check is treated as a proven missing/duplicate result without investigation.\n- Addition checks look for the new function name; deletion checks look for absence of its JSON name field. These are smoke assertions, not an exhaustive graph cleanup oracle.\n- CLI `verify` success and SQLite `PRAGMA integrity_check` are recorded. These do not prove semantic graph completeness.\n- Schema definitions were captured independently. Cross-version migration and downgrade compatibility: **NOT TESTED**; disposable indexes were created separately.\n- Missing/duplicated results, ranking semantics and tie ordering: exact-output mismatches require review; no complete ground-truth oracle.\n- Cancellation during active writes: **NOT TESTED**. Concurrent clients/writes: **NOT TESTED**. Long-running RSS stabilization: **NOT TESTED**. Upstream Rust regression tests: **NOT TESTED**.\n\n'
for x in fail:text+=f"- Unsuccessful check: `{x['name']}` — {x['detail']}\n"
text+='\n## 5. Performance regressions\n\nExploratory supported regressions below require a slowdown exceeding 5% and a bootstrap interval entirely below zero. This is not a multiple-comparison-adjusted confirmatory result. Reproducibility is limited to the repetitions on this machine; an independent rerun has not been performed. Impact is slower response/index completion for the named workload.\n\n'
text+=table(reg) if reg else 'No slowdown met this exploratory threshold. This does not prove absence of regressions.'
text+='\n\nSuspected slowdowns exceeding 5% with intervals including zero:\n\n'+(table(sus) if sus else 'None in this run.')
text+='''

## 6. Known limitations

- Both executables report the same release version. SHA-256 distinguishes them but does not establish their upstream commit or build options. No promotion decision can rely on an assumed Beta source revision.
- Source files were relocated under short hashed directories to avoid Windows path limits. File contents are unchanged, but include/import path resolution and project discovery can differ from the original repository. This is a hybrid exported corpus, not a production checkout.
- Five index repetitions and fifteen CLI repetitions provide limited statistical power. Concurrent system activity, power management, filesystem cache, antivirus, thermal state and fixed workload order are uncontrolled. No across-machine replication.
- CLI/MCP timing includes application behavior beyond SQL. Internal SQL plans, bundled-libSQL equivalence, write-lock/rollback behavior and per-optimization attribution cannot be established from these measurements.
- Public issue #786's reported upstream tests are background information, not tests executed here. No source build or cross-platform CI verification was performed.
- Two persistent servers with serial requests are not a production concurrency stress test. Request-boundary RSS and sampled CLI RSS do not establish true peak memory or a stable long-run plateau.
- No large all-matching FTS stress corpus, write cancellation, migration/downgrade, crash recovery or source-level regression suite was executed.

## 7. Final recommendation

**NEEDS ADDITIONAL VALIDATION**

Do not promote on this evidence alone. Confirm the binary build commits, review exact-output differences and any measured slowdowns, then run representative full-path production corpora, active-write cancellation, concurrent workloads and a long RSS soak. Timing improvements on selected rows cannot compensate for these unresolved correctness and coverage gaps.

## 8. Issue closure recommendation

**RECOMMEND KEEPING #786 OPEN**

The [public tracking issue](https://github.com/aovestdipaperino/tokensave/issues/786) requires broader Beta evaluation and cross-platform CI observation before proposing promotion; it explicitly keeps tracking/performance work open. This run does not verify those completion requirements. The unavailable source-level validation, unverified build provenance and production stability gaps prevent a defensible closure recommendation. No comment was posted and no issue state was changed.

## 9. Reproduction instructions

Run from the delivered directory in PowerShell. Python 3 with psutil is required (the executed environment used Python 3.14.7 and psutil 7.2.2). No GitHub CLI, authentication, SDK, source build or network download is required for the runner. Standard Git reads the configured local commit. Review/edit the two binary paths and repository path in the JSON if moving the package.

```powershell
Set-Location 'C:\\Users\\AndreaTontini\\Desktop\\e-shock_b-challenger\\tokensave-validation-786'
python -c "import psutil; print(psutil.__version__)"
Get-Content .\\benchmark-config.json
powershell -NoProfile -ExecutionPolicy Bypass -File .\\benchmark.ps1 -Config .\\benchmark-config.json
python .\\summarize.py
Get-Content -Raw .\\ISSUE-786-VALIDATION.md
Start-Process .\\benchmark-report.html
```

The runner recreates only its own `work/stable` and `work/beta` directories. It overwrites the output CSV and numbered raw logs; archive this completed evidence package before a separate historical run. It reuses `corpus/` only when the corpus manifest exists. Each run records binary hashes. Query arguments, limits, repetitions, process timeout and CPU profile are explicit in the runner/config. Warmups are omitted from raw measurements. `summarize.py` deterministically regenerates statistics and reports from the CSV/check logs and metadata.

## 10. Evidence and attachments

- `benchmark-results.csv`: each measured operation, timing, sampled CPU/RSS, exit/error status and raw-log path. The `database_bytes` row uses bytes in the historical `elapsed_ms` field; the summary's unit column disambiguates it.
- `benchmark-summary.csv`: medians, differences, percentage improvements, bootstrap intervals, p95/p99 and resources.
- `benchmark-config.json`, `benchmark.ps1`, `benchmark.py`, `summarize.py`: exact reproducible configuration, runner and report generator.
- `binaries.json`: versions, SHA-256 and start timestamp.
- `environment.json`: hardware, OS and power metadata.
- `corpus-manifest.json`, `corpus/`: committed source mapping/hashes and actual synthetic fixture corpus.
- `correctness.json`: individual executed checks, including unsuccessful comparisons.
- `logs/`: complete request/command outputs and errors, schema captures and server stderr. Logs/corpus may contain proprietary firmware source; review before publicly attaching them.
- `issue-786.json`: unauthenticated public API snapshot of issue requirements.
- `run.log`: execution progress and errors.
- `benchmark-report.html`: standalone visual summary.
- `README.md`: setup/reproduction and interpretation notes.

All new timing/resource counts come from the raw CSV or its deterministic summary. No unexecuted test is reported as PASS. Upstream measurements were not mixed into this dataset.
'''
extra='\n### Additional bounded stability checks\n\n'
for label,e in extended.items():
    rss=[x['rss'] for x in e['rss_samples']]
    extra+=f"- {label.title()}: {e['requests']} requests over {e['soak_seconds']:.3f} s, {e['errors']} response errors. RSS across sampled request boundaries: first {rss[0] if rss else 'N/A'}, last {rss[-1] if rss else 'N/A'}, maximum {max(rss) if rss else 'N/A'} bytes. Four concurrent reader processes each issued 30 requests against the same index; errors: {sum(map(len,e['concurrent_errors']))}.\n"
    extra+=f"- {label.title()} forced-termination probe: termination after {e['cancellation']['delay_seconds']} s; process was still running: {e['cancellation']['terminated_running_process']}; exit code {e['cancellation']['exit_code']}. Subsequent forced reindex and verify outcomes appear in `correctness.json` and raw CSV. The active database-write phase was not confirmed, and this is not graceful MCP cancellation.\n"
extra+='\nSource: `extended-results.json`, `extended-run.log` and cancellation logs. One minute is a bounded smoke soak, insufficient to establish long-run RSS stabilization. Concurrency covers multiple read-only server processes, not concurrent requests on one server or simultaneous writers.\n'
by_name={x['workload']:x for x in summ}
scope=by_name['mcp_complexity_scoped'];rare=by_name['mcp_fts_rare'];db=by_name['database_bytes']
concrete=f"The clearest query improvement is scoped complexity over MCP: {scope['stable_p50']:.3f} → {scope['beta_p50']:.3f} ms ({scope['improvement_percent']:.1f}% faster). Rare-term MCP search is {rare['stable_p50']:.3f} → {rare['beta_p50']:.3f} ms ({-rare['improvement_percent']:.1f}% slower); its exploratory interval excludes zero. Most other query differences are inconclusive. Database files are {db['improvement_percent']:.2f}% smaller in Beta. Sampled memory observations do not establish an overall memory-efficiency advantage. Initial indexing is approximately equivalent within broad uncertainty; no reliable overall speed advantage is established.\n\n"
text=text.replace('This is an external Windows binary comparison,',concrete+'This is an external Windows binary comparison,',1)
text=text.replace('## 5. Performance regressions',extra+'\n## 5. Performance regressions')
text=text.replace('Concurrent clients/writes: **NOT TESTED**.','Concurrent readers: bounded checks below; concurrent writers: **NOT TESTED**.')
text=text.replace('Two persistent servers with serial requests are not a production concurrency stress test.','The paired serial benchmark and four-reader smoke check are not a production concurrency stress test.')
text=text.replace('No large all-matching FTS stress corpus, write cancellation, migration/downgrade, crash recovery or source-level regression suite was executed.','No large all-matching FTS stress corpus, confirmed active-write cancellation, controlled migration/downgrade or source-level regression suite was executed. Forced-termination recovery is covered only by the bounded checks above.')
text=text.replace('The runner recreates only its own `work/stable` and `work/beta` directories.','The runner recreates only its named Stable/Beta/cancellation directories under `%TEMP%/tokensave-issue786-work/`, outside the repository.')
text=text.replace('`benchmark.py`, `summarize.py`','`benchmark.py`, `extended.py`, `summarize.py`')
text=text.replace('- `run.log`: execution progress and errors.','- `run.log`, `extended-run.log`, `extended-results.json`: execution progress and bounded stability evidence.\n- `excluded-pilot/`: invalid nested-workspace pilot, excluded entirely from all statistics. Beta discovered the parent index and migrated it to schema 20. `parent-index-recovery.log` records a fresh Stable rebuild, verification and restoration at the original database path; the pre-recovery database is preserved. Tracked firmware files were not modified. The shortened-path corpus also follows an earlier Git path-length export failure.\n- `repair-parent-index.py`: one-time recovery script, not part of benchmark reproduction.')
text=text.replace('There are ', 'For the 12 query cases, all 24 CLI/MCP exact ordered-output comparisons matched after normalization. This establishes equivalence only for the exercised outputs, not all functionality.\n\nThere are ',1)
text=text.replace('review exact-output differences and any measured slowdowns','investigate any measured slowdowns and broaden correctness coverage')
text=text.replace('Addition checks look for the new function name; deletion checks look for absence of its JSON name field.','Addition checks require the new function name and count 1; deletion checks require its absence and count 0. These assertions were also re-evaluated against every saved addition/deletion response.')
(R/'ISSUE-786-VALIDATION.md').write_text(text,encoding='utf-8')
bars=''
for x in summ:
    if x['unit']!='ms':continue
    scale=max(x['stable_p50'],x['beta_p50']) or 1
    bars+=f"<article><h3>{html.escape(x['workload'])}</h3><div class='stable' style='width:{x['stable_p50']/scale*95}%'>Stable {x['stable_p50']:.3f} ms</div><div class='beta' style='width:{x['beta_p50']/scale*95}%'>Beta {x['beta_p50']:.3f} ms</div><p>Improvement {x['improvement_percent']:+.1f}%; 95% CI of difference [{x['improvement_ci95_low']:.3f}, {x['improvement_ci95_high']:.3f}] ms; n={x['n']}</p></article>"
(R/'benchmark-report.html').write_text("<!doctype html><html lang='en'><meta charset='utf-8'><title>Issue 786 Windows validation</title><style>body{font:16px system-ui;max-width:1100px;margin:40px auto;background:#f5f7fb;color:#172235}article{background:white;padding:18px;margin:16px 0;border-radius:10px}article div{padding:8px;margin:5px 0;min-width:180px;color:white;box-sizing:border-box}.stable{background:#465c80}.beta{background:#167967}pre{white-space:pre-wrap;background:white;padding:24px}a{color:#167967}</style><h1>TokenSave Windows validation — #786</h1><p><strong>NEEDS ADDITIONAL VALIDATION · KEEP #786 OPEN</strong></p><p>Median elapsed time; each chart has its own scale. Exploratory intervals, not production-tail guarantees. Full methodology and gaps below.</p><a href='benchmark-summary.csv'>Summary CSV</a> · <a href='benchmark-results.csv'>Raw CSV</a> · <a href='ISSUE-786-VALIDATION.md'>Copy-ready Markdown</a>"+bars+'<h2>Complete report</h2><pre>'+html.escape(text)+'</pre></html>',encoding='utf-8')
print('Generated report;',len(raw),'measurements;',len(checks),'checks;',len(fail),'unsuccessful;',len(reg),'exploratory regressions')
