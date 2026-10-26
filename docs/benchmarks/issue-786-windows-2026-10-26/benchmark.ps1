param([string]$Config = "$PSScriptRoot\benchmark-config.json")
$ErrorActionPreference = 'Stop'
python "$PSScriptRoot\benchmark.py" --config $Config
if ($LASTEXITCODE -ne 0) { throw "Benchmark failed: $LASTEXITCODE" }
python "$PSScriptRoot\extended.py" --config $Config
if ($LASTEXITCODE -ne 0) { throw "Extended checks failed: $LASTEXITCODE" }
python "$PSScriptRoot\summarize.py"
if ($LASTEXITCODE -ne 0) { throw "Report generation failed: $LASTEXITCODE" }
