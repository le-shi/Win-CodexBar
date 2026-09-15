@echo off
setlocal EnableExtensions DisableDelayedExpansion

set "APP_DIR=%~dp0"
set "EXE=%APP_DIR%codexbar-cli.exe"
set "TOKEN_FILE=%APP_DIR%metrics.token"
set "PROVIDERS_FILE=%TEMP%\codexbar-providers-%RANDOM%-%RANDOM%.txt"
set "DOWNLOAD_BASE=https://github.com/le-shi/Win-CodexBar/releases/download/metrics-v0.56.8-r3"
set "DOWNLOAD_URL=%DOWNLOAD_BASE%/codexbar-cli.exe"
set "CHECKSUM_URL=%DOWNLOAD_BASE%/codexbar-cli.exe.sha256"
set "TEMP_EXE=%APP_DIR%codexbar-cli.download-%RANDOM%-%RANDOM%.exe"
set "TEMP_SHA=%APP_DIR%codexbar-cli.exe.sha256.download-%RANDOM%-%RANDOM%"
set "FAIL_REASON="

set "CODEXBAR_EXE=%EXE%"
set "CODEXBAR_DOWNLOAD_URL=%DOWNLOAD_URL%"
set "CODEXBAR_CHECKSUM_URL=%CHECKSUM_URL%"
set "CODEXBAR_TEMP_EXE=%TEMP_EXE%"
set "CODEXBAR_TEMP_SHA=%TEMP_SHA%"
echo Checking codexbar-cli.exe release checksum...
powershell.exe -NoLogo -NoProfile -NonInteractive -Command ^
  "$ErrorActionPreference='Stop'; function Get-Sha256([string]$path) { $stream=[IO.File]::OpenRead($path); $sha=[Security.Cryptography.SHA256]::Create(); try { -join ($sha.ComputeHash($stream) | ForEach-Object { $_.ToString('x2') }) } finally { $sha.Dispose(); $stream.Dispose() } }; Invoke-WebRequest -UseBasicParsing -Uri $env:CODEXBAR_CHECKSUM_URL -OutFile $env:CODEXBAR_TEMP_SHA; $lines=@([IO.File]::ReadAllLines($env:CODEXBAR_TEMP_SHA) | Where-Object { $_.Length -gt 0 }); if($lines.Count -ne 1 -or $lines[0] -notmatch '^([0-9A-Fa-f]{64})  codexbar-cli[.]exe$') { throw 'Invalid codexbar-cli.exe.sha256 release asset' }; $expected=$Matches[1]; $installed=(Test-Path -LiteralPath $env:CODEXBAR_EXE) -and ((Get-Sha256 $env:CODEXBAR_EXE) -ieq $expected); if(-not $installed) { Invoke-WebRequest -UseBasicParsing -Uri $env:CODEXBAR_DOWNLOAD_URL -OutFile $env:CODEXBAR_TEMP_EXE; $actual=Get-Sha256 $env:CODEXBAR_TEMP_EXE; if($actual -ine $expected) { throw ('SHA-256 mismatch: ' + $actual) }; $help=& $env:CODEXBAR_TEMP_EXE serve --help 2>&1; if($LASTEXITCODE -ne 0 -or -not ($help -match '--metrics')) { throw 'Downloaded executable does not support serve --metrics' }; Move-Item -LiteralPath $env:CODEXBAR_TEMP_EXE -Destination $env:CODEXBAR_EXE -Force }; Remove-Item -LiteralPath $env:CODEXBAR_TEMP_SHA -Force"
if errorlevel 1 (
  set "FAIL_REASON=Could not download or verify the metrics-v0.56.8-r3 codexbar-cli.exe."
  goto :fail
)

"%EXE%" serve --help 2>&1 | "%SystemRoot%\System32\findstr.exe" /C:"--metrics" >nul
if errorlevel 1 (
  set "FAIL_REASON=This codexbar-cli.exe does not support serve --metrics."
  goto :fail
)

"%EXE%" config providers >"%PROVIDERS_FILE%"
if errorlevel 1 (
  set "FAIL_REASON=Could not read the CodexBar provider configuration."
  goto :fail
)

for /f "usebackq tokens=1,2 delims=: " %%A in ("%PROVIDERS_FILE%") do (
  if /I "%%B"=="enabled" if /I not "%%A"=="codex" (
    echo Disabling provider %%A...
    "%EXE%" config disable "%%A"
    if errorlevel 1 (
      set "FAIL_REASON=Could not disable a non-Codex provider."
      goto :fail
    )
  )
)

"%EXE%" config enable codex
if errorlevel 1 (
  set "FAIL_REASON=Could not enable the Codex provider."
  goto :fail
)

"%EXE%" config validate
if errorlevel 1 (
  set "FAIL_REASON=CodexBar configuration validation failed."
  goto :fail
)

set "CODEXBAR_TOKEN_FILE=%TOKEN_FILE%"
powershell.exe -NoLogo -NoProfile -NonInteractive -Command ^
  "$ErrorActionPreference='Stop'; $path=$env:CODEXBAR_TOKEN_FILE; if(-not (Test-Path -LiteralPath $path)) { $bytes=New-Object byte[] 32; $rng=[Security.Cryptography.RandomNumberGenerator]::Create(); try { $rng.GetBytes($bytes) } finally { $rng.Dispose() }; $token=-join ($bytes | ForEach-Object { $_.ToString('x2') }); [IO.File]::WriteAllText($path,$token,[Text.Encoding]::ASCII) }; $raw=[IO.File]::ReadAllBytes($path); $invalid=@($raw | Where-Object { -not (($_ -ge 48 -and $_ -le 57) -or ($_ -ge 65 -and $_ -le 70) -or ($_ -ge 97 -and $_ -le 102)) }); if($raw.Length -ne 64 -or $invalid.Count -ne 0) { throw 'metrics.token must contain exactly 64 ASCII hexadecimal bytes with no BOM or newline' }; $currentSid=[Security.Principal.WindowsIdentity]::GetCurrent().User; $systemSid=[Security.Principal.SecurityIdentifier]'S-1-5-18'; $acl=New-Object Security.AccessControl.FileSecurity; $acl.SetAccessRuleProtection($true,$false); $acl.SetOwner($currentSid); [void]$acl.AddAccessRule((New-Object Security.AccessControl.FileSystemAccessRule($currentSid,[Security.AccessControl.FileSystemRights]::Read,[Security.AccessControl.AccessControlType]::Allow))); [void]$acl.AddAccessRule((New-Object Security.AccessControl.FileSystemAccessRule($systemSid,[Security.AccessControl.FileSystemRights]::FullControl,[Security.AccessControl.AccessControlType]::Allow))); Set-Acl -LiteralPath $path -AclObject $acl; $readBack=Get-Acl -LiteralPath $path; $allowed=@($currentSid.Value,$systemSid.Value); $seen=@(); foreach($rule in $readBack.Access) { $sid=$rule.IdentityReference.Translate([Security.Principal.SecurityIdentifier]).Value; if($rule.IsInherited -or $rule.AccessControlType -ne 'Allow' -or $sid -notin $allowed) { throw ('Unexpected metrics.token ACL entry: ' + $sid) }; $seen += $sid }; if(-not $readBack.AreAccessRulesProtected -or $seen.Count -ne 2 -or $currentSid.Value -notin $seen -or $systemSid.Value -notin $seen) { throw 'metrics.token ACL verification failed' }"
if errorlevel 1 (
  set "FAIL_REASON=Could not create or secure metrics.token."
  goto :fail
)

del /q "%PROVIDERS_FILE%" >nul 2>&1
echo.
echo Codex-only metrics setup completed.
echo Executable: "%EXE%"
echo Token file: "%TOKEN_FILE%"
echo Run start-codex-metrics.cmd from this directory.
endlocal
exit /b 0

:fail
del /q "%PROVIDERS_FILE%" >nul 2>&1
del /q "%TEMP_EXE%" >nul 2>&1
del /q "%TEMP_SHA%" >nul 2>&1
>&2 echo ERROR: %FAIL_REASON%
endlocal
exit /b 1
