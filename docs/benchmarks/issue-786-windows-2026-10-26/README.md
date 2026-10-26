# Issue #786 Windows binary validation

Open `ISSUE-786-VALIDATION.md` for the GitHub-ready report or `benchmark-report.html` for charts and the full report. The results were subsequently prepared for user-authorized publication.

Requirements: Windows PowerShell, Python 3, psutil (executed version 7.2.2), standard Git, the local firmware commit and the two installed Windows binaries. The benchmark runner does not use GitHub CLI or authentication.

```powershell
Set-Location 'C:\Users\AndreaTontini\Desktop\e-shock_b-challenger\tokensave-validation-786'
python -c "import psutil; print(psutil.__version__)"
Get-Content .\benchmark-config.json
powershell -NoProfile -ExecutionPolicy Bypass -File .\benchmark.ps1 -Config .\benchmark-config.json
python .\summarize.py
```

Adjust binary/repository paths in the configuration when moving this package. The runner recreates its own named work directories under `%TEMP%/tokensave-issue786-work`, outside the repository, and overwrites generated measurements: archive a completed run before a separate historical run. The corpus is reused when its manifest exists. The runner includes two warmups, alternating binary order, five index rounds, fifteen CLI rounds and one hundred persistent MCP rounds per query, followed by one-minute soaks, concurrent-reader checks and forced-termination recovery checks. Do not change parameters without reporting the change. `benchmark.ps1` runs all three Python stages; the explicit summarizer command above can also regenerate reports independently.

An excluded initial pilot revealed Beta ancestor-project discovery and migrated the parent index. The parent was rebuilt with Stable and verified at its original database path; the pre-recovery database is preserved under `excluded-pilot/`. Recovery is not a benchmark step. Generated files are locally Git-ignored to keep subsequent parent indexing from including the benchmark corpus.

Inspect raw logs via each CSV row's `log` field. `database_bytes` uses bytes in the raw `elapsed_ms` field; the summary labels its unit explicitly. CPU uses one-core percent; observed RSS is sampled, not guaranteed peak memory. Bootstrap intervals are exploratory and not corrected for multiple comparisons. Short runs do not establish production stability or equivalence.

The private firmware corpus and request logs can contain proprietary source: the Markdown report is intended for copying; review raw attachments before public sharing.

## Public evidence package

Public upload scope: this package contains the report, numeric measurements, summaries, checks, environment metadata and reproduction scripts. The private firmware corpus, source-bearing request logs, corpus manifest and database backups are retained locally and are not included. Reproducing this exact corpus requires access to the private repository; the CSV log paths refer to retained local evidence. Benchmark execution used no GitHub authentication; this subsequent upload was explicitly requested by the user and used existing WSL GitHub authentication.

