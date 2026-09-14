@echo off
setlocal EnableExtensions DisableDelayedExpansion

rem Override these variables before launching when the host or port differs.
if not defined CODEXBAR_BIND_IP set "CODEXBAR_BIND_IP=192.168.13.111"
if not defined CODEXBAR_PORT set "CODEXBAR_PORT=8080"
if not defined CODEXBAR_REFRESH_INTERVAL set "CODEXBAR_REFRESH_INTERVAL=300"

set "APP_DIR=%~dp0"
set "EXE=%APP_DIR%codexbar-cli.exe"
set "TOKEN_FILE=%APP_DIR%metrics.token"
set "FAIL_REASON="

if not exist "%EXE%" (
  set "FAIL_REASON=Missing codexbar-cli.exe. Run setup-codex-only.cmd first."
  goto :fail
)
if not exist "%TOKEN_FILE%" (
  set "FAIL_REASON=Missing metrics.token. Run setup-codex-only.cmd first."
  goto :fail
)

set "CODEXBAR_TOKEN_FILE=%TOKEN_FILE%"
powershell.exe -NoLogo -NoProfile -NonInteractive -Command ^
  "$ErrorActionPreference='Stop'; $raw=[IO.File]::ReadAllBytes($env:CODEXBAR_TOKEN_FILE); $invalid=@($raw | Where-Object { -not (($_ -ge 48 -and $_ -le 57) -or ($_ -ge 65 -and $_ -le 70) -or ($_ -ge 97 -and $_ -le 102)) }); if($raw.Length -ne 64 -or $invalid.Count -ne 0) { throw 'metrics.token must contain exactly 64 ASCII hexadecimal bytes with no BOM or newline' }; $currentSid=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value; $systemSid='S-1-5-18'; $allowed=@($currentSid,$systemSid); $acl=Get-Acl -LiteralPath $env:CODEXBAR_TOKEN_FILE; $seen=@(); foreach($rule in $acl.Access) { $sid=$rule.IdentityReference.Translate([Security.Principal.SecurityIdentifier]).Value; if($rule.IsInherited -or $rule.AccessControlType -ne 'Allow' -or $sid -notin $allowed) { throw ('Unexpected metrics.token ACL entry: ' + $sid) }; $seen += $sid }; if(-not $acl.AreAccessRulesProtected -or $seen.Count -ne 2 -or $currentSid -notin $seen -or $systemSid -notin $seen) { throw 'metrics.token ACL must contain only the current user and SYSTEM' }"
if errorlevel 1 (
  set "FAIL_REASON=metrics.token is invalid or has unsafe ACLs."
  goto :fail
)

set "CODEXBAR_EXE=%EXE%"
powershell.exe -NoLogo -NoProfile -NonInteractive -Command ^
  "$ErrorActionPreference='Stop'; $lines=@(& $env:CODEXBAR_EXE config providers 2>&1); if($LASTEXITCODE -ne 0) { throw 'Could not read provider configuration' }; $enabled=@($lines | ForEach-Object { $line=[string]$_; if($line -match '^([a-z0-9-]+): enabled(?:\s|$)') { $Matches[1] } }); if($enabled.Count -ne 1 -or $enabled[0] -ne 'codex') { throw ('Expected only codex enabled; found: ' + ($enabled -join ', ')) }"
if errorlevel 1 (
  set "FAIL_REASON=Provider configuration is not Codex-only. Run setup-codex-only.cmd."
  goto :fail
)

powershell.exe -NoLogo -NoProfile -NonInteractive -Command ^
  "$ErrorActionPreference='Stop'; $ip=[Net.IPAddress]::Parse($env:CODEXBAR_BIND_IP); if($ip.AddressFamily -ne [Net.Sockets.AddressFamily]::InterNetwork) { throw 'CODEXBAR_BIND_IP must be IPv4' }; $assigned=@([Net.NetworkInformation.NetworkInterface]::GetAllNetworkInterfaces() | ForEach-Object { $_.GetIPProperties().UnicastAddresses } | ForEach-Object { $_.Address.ToString() }); if($ip.ToString() -notin $assigned) { throw ('IPv4 address is not assigned to this machine: ' + $ip) }; $probe=[Net.Sockets.TcpListener]::new($ip,[int]$env:CODEXBAR_PORT); try { $probe.Start() } finally { $probe.Stop() }"
if errorlevel 1 (
  set "FAIL_REASON=The bind address is unavailable or the port is already in use."
  goto :fail
)

set "CODEXBAR_DASHBOARD_TOKEN="
set /p "CODEXBAR_DASHBOARD_TOKEN="<"%TOKEN_FILE%"
if not defined CODEXBAR_DASHBOARD_TOKEN (
  set "FAIL_REASON=Could not read a Token from metrics.token."
  goto :fail
)

echo Starting Codex-only metrics at http://%CODEXBAR_BIND_IP%:%CODEXBAR_PORT%/metrics
echo Press Ctrl+C to stop.
"%EXE%" serve ^
  --host "%CODEXBAR_BIND_IP%" ^
  --port "%CODEXBAR_PORT%" ^
  --refresh-interval "%CODEXBAR_REFRESH_INTERVAL%" ^
  --identity redacted ^
  --metrics ^
  --allow-plain-http ^
  --verbose ^
  --no-color

set "RUN_RESULT=%ERRORLEVEL%"
set "CODEXBAR_DASHBOARD_TOKEN="
endlocal & exit /b %RUN_RESULT%

:fail
>&2 echo ERROR: %FAIL_REASON%
endlocal
exit /b 1
