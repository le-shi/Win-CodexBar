@echo off
setlocal EnableExtensions DisableDelayedExpansion

if not defined CODEXBAR_BIND_IP set "CODEXBAR_BIND_IP=192.168.13.111"
if not defined CODEXBAR_PORT set "CODEXBAR_PORT=8081"

set "TOKEN_FILE=%~dp0metrics.token"
if not exist "%TOKEN_FILE%" (
  >&2 echo ERROR: Missing "%TOKEN_FILE%".
  endlocal
  exit /b 1
)

set "CODEXBAR_TOKEN_FILE=%TOKEN_FILE%"
set "CODEXBAR_METRICS_URL=http://%CODEXBAR_BIND_IP%:%CODEXBAR_PORT%/metrics"
powershell.exe -NoLogo -NoProfile -NonInteractive -Command ^
  "$ErrorActionPreference='Stop'; $raw=[IO.File]::ReadAllBytes($env:CODEXBAR_TOKEN_FILE); $invalid=@($raw | Where-Object { -not (($_ -ge 48 -and $_ -le 57) -or ($_ -ge 65 -and $_ -le 70) -or ($_ -ge 97 -and $_ -le 102)) }); if($raw.Length -ne 64 -or $invalid.Count -ne 0) { throw 'metrics.token must contain exactly 64 ASCII hexadecimal bytes with no BOM or newline' }; $token=[Text.Encoding]::ASCII.GetString($raw); $response=Invoke-WebRequest -UseBasicParsing -Uri $env:CODEXBAR_METRICS_URL -Headers @{ Authorization=('Bearer ' + $token) }; $body=[string]$response.Content; $lines=@($body -split '\r?\n'); $quote=[char]34; $enabledLine='codexbar_provider_enabled{provider=' + $quote + 'codex' + $quote + '} 1'; $resetPrefix='codexbar_reset_credits_available{provider=' + $quote + 'codex' + $quote + '} '; if($response.StatusCode -ne 200) { throw ('HTTP status ' + $response.StatusCode) }; if($lines -notcontains 'codexbar_up 1') { throw 'codexbar_up is not 1' }; if($lines -notcontains $enabledLine) { throw 'Codex provider is not enabled' }; $resetLines=@($lines | Where-Object { $_.StartsWith($resetPrefix,[StringComparison]::Ordinal) }); if($resetLines.Count -ne 1) { throw 'Expected exactly one Codex reset credits metric' }; $resetValue=$resetLines[0].Substring($resetPrefix.Length); if($resetValue -notmatch '^(?:-1|[0-9]+)$') { throw ('Invalid Codex reset credits value: ' + $resetValue) }; $providerPattern='provider=' + $quote + '([a-z0-9-]+)' + $quote; $providers=@([regex]::Matches($body,$providerPattern) | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique); $unexpected=@($providers | Where-Object { $_ -ne 'codex' }); if($unexpected.Count -gt 0) { throw ('Unexpected provider metrics: ' + ($unexpected -join ', ')) }; $lines | Where-Object { $_ -match '^codexbar_' }"

set "TEST_RESULT=%ERRORLEVEL%"
if not "%TEST_RESULT%"=="0" >&2 echo ERROR: Codex-only metrics verification failed.
endlocal & exit /b %TEST_RESULT%
