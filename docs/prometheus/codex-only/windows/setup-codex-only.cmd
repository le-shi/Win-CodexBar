@echo off
setlocal EnableExtensions DisableDelayedExpansion

set "APP_DIR=%~dp0"
set "EXE=%APP_DIR%codexbar-cli.exe"
set "TOKEN_FILE=%APP_DIR%metrics.token"
set "PROVIDERS_FILE=%TEMP%\codexbar-providers-%RANDOM%-%RANDOM%.txt"
set "DOWNLOAD_URL=https://github.com/le-shi/Win-CodexBar/releases/download/metrics-v0.56.8-r2/codexbar-cli.exe"
set "EXPECTED_SHA256=0D926D7597ABD27F8DC6D3B168062502AAA69AC36F77D6B1F8859DF809642661"
set "FAIL_REASON="

if not exist "%EXE%" (
  echo Downloading codexbar-cli.exe...
  set "CODEXBAR_EXE=%EXE%"
  set "CODEXBAR_DOWNLOAD_URL=%DOWNLOAD_URL%"
  powershell.exe -NoLogo -NoProfile -NonInteractive -Command ^
    "$ErrorActionPreference='Stop'; Invoke-WebRequest -UseBasicParsing -Uri $env:CODEXBAR_DOWNLOAD_URL -OutFile $env:CODEXBAR_EXE"
  if errorlevel 1 (
    set "FAIL_REASON=Could not download codexbar-cli.exe."
    goto :fail
  )
)

set "CODEXBAR_EXE=%EXE%"
set "CODEXBAR_EXPECTED_SHA256=%EXPECTED_SHA256%"
powershell.exe -NoLogo -NoProfile -NonInteractive -Command ^
  "$ErrorActionPreference='Stop'; $stream=[IO.File]::OpenRead($env:CODEXBAR_EXE); $sha=[Security.Cryptography.SHA256]::Create(); try { $actual=-join ($sha.ComputeHash($stream) | ForEach-Object { $_.ToString('x2') }) } finally { $sha.Dispose(); $stream.Dispose() }; if($actual -ine $env:CODEXBAR_EXPECTED_SHA256) { throw ('SHA-256 mismatch: ' + $actual) }"
if errorlevel 1 (
  set "FAIL_REASON=codexbar-cli.exe failed SHA-256 verification."
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
>&2 echo ERROR: %FAIL_REASON%
endlocal
exit /b 1
