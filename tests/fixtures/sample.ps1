# Application configuration module.

Import-Module ActiveDirectory
. .\Utils.ps1

# Maximum retry count.
[int]$MaxRetries = 3
# Default port for connections.
[int]$DefaultPort = 8080

<#
.SYNOPSIS
    Logs a message with the given level.
.PARAMETER Level
    The log level.
.PARAMETER Message
    The message to log.
#>
function Write-Log {
    param(
        [string]$Level,
        [string]$Message
    )
    Write-Host "[$(Get-Date)] [$Level] $Message"
}

# Validates the configuration.
function Test-Config {
    param(
        [string]$Host,
        [int]$Port
    )
    if ([string]::IsNullOrEmpty($Host)) {
        Write-Log -Level "ERROR" -Message "Host is not set"
        return $false
    }
    if ($Port -lt 1 -or $Port -gt 65535) {
        Write-Log -Level "ERROR" -Message "Invalid port: $Port"
        return $false
    }
    return $true
}

# Connects to the remote server.
function Connect-Server {
    param(
        [string]$HostName,
        [int]$Port = $DefaultPort
    )
    Write-Log -Level "INFO" -Message "Connecting to ${HostName}:${Port}"
    for ($i = 1; $i -le $MaxRetries; $i++) {
        try {
            Test-Connection -ComputerName $HostName -Port $Port -ErrorAction Stop
            Write-Log -Level "INFO" -Message "Connected successfully"
            return $true
        } catch {
            Write-Log -Level "WARN" -Message "Retry $i/$MaxRetries"
            Start-Sleep -Seconds 1
        }
    }
    return $false
}

# Disconnects from the server.
function Disconnect-Server {
    Write-Log -Level "INFO" -Message "Disconnecting"
}

# Main entry point.
function Main {
    if (Test-Config -Host $env:HOST -Port $env:PORT) {
        Connect-Server -HostName $env:HOST -Port $env:PORT
        Disconnect-Server
    }
}

Main
