@echo off
REM Application startup script.

REM Maximum retry count.
set MAX_RETRIES=3
REM Default port.
set DEFAULT_PORT=8080

call :Main %*
goto :EOF

REM Logs a message with timestamp.
:Log
    echo [%DATE% %TIME%] [%~1] %~2
    goto :EOF

REM Validates the configuration.
:ValidateConfig
    if "%HOST%"=="" (
        call :Log "ERROR" "HOST is not set"
        exit /b 1
    )
    call :Log "INFO" "Config valid"
    exit /b 0

REM Connects to the remote server.
:Connect
    call :Log "INFO" "Connecting to %HOST%:%DEFAULT_PORT%"
    for /l %%i in (1,1,%MAX_RETRIES%) do (
        ping -n 1 %HOST% >nul 2>&1
        if not errorlevel 1 (
            call :Log "INFO" "Connected"
            exit /b 0
        )
        call :Log "WARN" "Retry %%i"
    )
    exit /b 1

REM Disconnects from the server.
:Disconnect
    call :Log "INFO" "Disconnecting"
    goto :EOF

REM Main entry point.
:Main
    call :ValidateConfig
    if errorlevel 1 exit /b 1
    call :Connect
    call :Disconnect
    goto :EOF
