#!/bin/bash

# Application configuration
readonly MAX_RETRIES=3
readonly DEFAULT_PORT=8080

source ./utils.sh

# Logs a message with timestamp.
log() {
    local level="$1"
    local message="$2"
    echo "[$(date)] [$level] $message"
}

# Validates the configuration.
validate_config() {
    if [ -z "$HOST" ]; then
        log "ERROR" "HOST is not set"
        return 1
    fi
    if [ "$PORT" -lt 1 ] || [ "$PORT" -gt 65535 ]; then
        log "ERROR" "Invalid port: $PORT"
        return 1
    fi
    return 0
}

# Connects to the remote server.
connect() {
    local host="$1"
    local port="${2:-$DEFAULT_PORT}"
    log "INFO" "Connecting to $host:$port"
    for i in $(seq 1 $MAX_RETRIES); do
        if curl -s "$host:$port" > /dev/null 2>&1; then
            log "INFO" "Connected successfully"
            return 0
        fi
        log "WARN" "Retry $i/$MAX_RETRIES"
        sleep 1
    done
    return 1
}

# Disconnects from the server.
disconnect() {
    log "INFO" "Disconnecting"
}

# Main entry point.
main() {
    validate_config
    connect "$HOST" "$PORT"
    local status=$?
    if [ $status -ne 0 ]; then
        log "ERROR" "Failed to connect"
        exit 1
    fi
    disconnect
}

main "$@"
