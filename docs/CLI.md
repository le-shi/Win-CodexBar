# CodexBar CLI (Windows)

Windows rewrite of upstream `docs/cli.md` for the **`codexbar`** binary built from `rust/`.
Upstream install paths (`/Applications`, Homebrew, Sparkle-bundled Helpers) do **not** apply.

## Install / build

```powershell
# From repo root
cargo build -p codexbar --release
# Source-build binary: target\release\codexbar.exe

cargo run -p codexbar -- --help
```

The Rust target is named `codexbar.exe`. Published Windows console assets and installer copies rename that binary to `codexbar-cli.exe`; `codexbar.exe` in an installed release is the tray application. There is no “Preferences → Install CLI” symlink flow like macOS.

## Configuration

CLI and desktop app share the same Windows config directory (see [CONFIGURATION.md](./CONFIGURATION.md)):

- Settings: `%AppData%\Roaming\CodexBar\settings.json`
- Manual cookies / API keys / token accounts: sibling files under that folder

```powershell
codexbar config path
codexbar config validate
codexbar config dump
```

## Commands (current)

Top-level (from `codexbar --help`):

| Command | Purpose |
|---------|---------|
| `usage` | Print usage from enabled providers (default-style workflow; also global `-p` / `-f`) |
| `cost` | Local token cost usage (Claude + Codex session scans; no web required for those) |
| `guard` | Gate automation on remaining quota for one provider |
| `diagnose` | Export safe provider diagnostics as JSON |
| `sessions` | List or focus local / SSH agent sessions |
| `serve` | HTTP JSON/dashboard server, with an opt-in Prometheus `/metrics` endpoint |
| `autostart` | Manage Windows boot auto-start |
| `account` | Token accounts for providers |
| `config` | validate / dump / providers / enable / disable / set-api-key / path |
| `hooks` | List, enable, disable, or test external hooks |

### Usage

```powershell
codexbar usage
codexbar usage -p claude -f json --pretty
codexbar usage -p all --status
codexbar usage --source auto   # auto | web | cli | oauth
codexbar usage --brief
```

Global-style flags (also on root help): `-p/--provider`, `-f/--format`, `--json`, `--pretty`, `--status`, `--all-accounts`, `--account`, `--no-credits`, `--source`, `--web-timeout`, `--brief`.

### Cost

```powershell
codexbar cost
codexbar cost -p codex -f json --pretty
```

Claude/Codex costs come from local session logs. Antigravity exposes local **token history only** through `cost`; dollar cost remains unknown rather than becoming a false `$0`. Other providers may differ; do not assume upstream Cursor dashboard cost behavior unless implemented in this tree.

Codex local-history scans use a 60-second scanner-side debounce for ordinary disk-cache reads. This is separate from the desktop provider refresh setting. With Adaptive refresh off, **Manual** (`refresh_interval_secs = 0`) disables the recurring desktop refresh timer, but it does not forbid startup/stale-aware reads, explicit refreshes, or pending Codex catch-up scans. Low Power Mode floors recurring automatic refreshes to 30 minutes; explicit/manual work remains immediate.

### Guard

```powershell
codexbar guard -p claude --min-remaining 10 --window session
codexbar guard -p codex --json --pretty --fail-open
```

Exit codes (stable intent): `0` ok, `1` below threshold, usage errors for bad args, unavailable when quota cannot be checked (`--fail-open` turns unavailable into `0`).

### Serve

```powershell
.\codexbar-cli.exe serve --port 8080
# Non-loopback binds need a dashboard token and --allow-plain-http (cleartext bearer).
# Prefer: $env:CODEXBAR_DASHBOARD_TOKEN = '<REPLACE_WITH_A_LONG_RANDOM_TOKEN>'
```

Typical endpoints: `/health`, `/usage`, `/cost` (and dashboard snapshot routes when enabled). Loopback default keeps local use simple; treat non-loopback as a threat-model choice (token on every request over HTTP).

#### Prometheus metrics

Download `codexbar-cli.exe` or `CodexBarCLI-metrics-v0.56.8-r2-windows-x64.zip` from the dedicated [`metrics-v0.56.8-r2` release](https://github.com/le-shi/Win-CodexBar/releases/tag/metrics-v0.56.8-r2). The standard `Finesssee/Win-CodexBar` v0.56.8 CLI does not contain `--metrics`. A source build of this branch also works after copying `target\release\codexbar.exe` to `codexbar-cli.exe`. Confirm the selected binary before deployment:

```powershell
.\codexbar-cli.exe serve --help | Select-String -SimpleMatch '--metrics'
```

Prometheus export is built into `serve`, but is disabled by default. Pass `--metrics` to enable `GET /metrics`; without the flag, that route returns `404`. When `CODEXBAR_DASHBOARD_TOKEN` or `--dashboard-token` is set, `/metrics` uses the same Bearer authentication as the other protected data routes.

The same listener also exposes the token-protected dashboard snapshot. Use `--identity redacted` for a monitoring deployment so those JSON routes omit personal identity fields. The Prometheus exporter already excludes those fields, so this option does not change `/metrics` series or labels.

For LAN access, bind the machine's actual LAN IPv4 address instead of `0.0.0.0`. The serve Host allowlist checks the configured address, so clients must use that same address in the URL. This example uses `192.168.13.111`; replace it with the address reported for the intended adapter:

```powershell
Get-NetIPAddress -AddressFamily IPv4 |
  Where-Object {
    $_.IPAddress -notlike '127.*' -and
    $_.AddressState -eq 'Preferred'
  } |
  Select-Object InterfaceAlias, IPAddress, PrefixLength

$env:CODEXBAR_DASHBOARD_TOKEN = '<REPLACE_WITH_A_LONG_RANDOM_TOKEN>'

.\codexbar-cli.exe serve `
  --host 192.168.13.111 `
  --port 8080 `
  --refresh-interval 300 `
  --identity redacted `
  --metrics `
  --allow-plain-http `
  --verbose
```

`--allow-plain-http` acknowledges that the Bearer token crosses the LAN without TLS. Keep the port on a trusted network and restrict it to the Prometheus server. In an elevated PowerShell, create an inbound rule with the real Prometheus server IP in place of `<PROMETHEUS_SERVER_IP>`:

```powershell
New-NetFirewallRule `
  -DisplayName 'Win-CodexBar metrics from Prometheus' `
  -Direction Inbound `
  -Action Allow `
  -Protocol TCP `
  -LocalAddress 192.168.13.111 `
  -LocalPort 8080 `
  -RemoteAddress '<PROMETHEUS_SERVER_IP>' `
  -Profile Domain,Private
```

Verify the listener and authenticated response on Windows:

```powershell
Get-NetTCPConnection -LocalAddress 192.168.13.111 -LocalPort 8080 -State Listen

curl.exe -fsS `
  -H "Authorization: Bearer $env:CODEXBAR_DASHBOARD_TOKEN" `
  http://192.168.13.111:8080/metrics |
  Select-String '^codexbar_'
```

##### Start at user logon

The provider configuration and credentials belong to the signed-in Windows user. Run the metrics process in that user's interactive session. Open an ordinary, non-elevated PowerShell as that user and run the following commands from the directory that contains `codexbar-cli.exe`. Keeping the files under that user's `%LOCALAPPDATA%` also avoids registering the task for a different administrator account used only for elevation. Use an elevated shell only for the firewall command shown earlier.

```powershell
$ErrorActionPreference = 'Stop'
$metricsDir = Join-Path $env:LOCALAPPDATA 'Win-CodexBar-Metrics'
$tokenFile = Join-Path $metricsDir 'metrics.token'
$launcher = Join-Path $metricsDir 'start-metrics.ps1'
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$currentUser = $identity.Name
$currentUserSid = $identity.User.Value

New-Item -ItemType Directory -Force -Path $metricsDir | Out-Null
& icacls.exe $metricsDir /inheritance:r /grant:r `
  "*${currentUserSid}:(OI)(CI)F" '*S-1-5-18:(OI)(CI)F' | Out-Null
if ($LASTEXITCODE -ne 0) {
  throw "Could not restrict ACLs on $metricsDir (icacls exit $LASTEXITCODE)."
}

Copy-Item -LiteralPath '.\codexbar-cli.exe' `
  -Destination (Join-Path $metricsDir 'codexbar-cli.exe') -Force

$bytes = New-Object byte[] 32
$rng = [Security.Cryptography.RandomNumberGenerator]::Create()
try { $rng.GetBytes($bytes) } finally { $rng.Dispose() }
$token = -join ($bytes | ForEach-Object { $_.ToString('x2') })
Set-Content -LiteralPath $tokenFile -Value $token -NoNewline -Encoding Ascii
& icacls.exe $tokenFile /inheritance:r /grant:r `
  "*${currentUserSid}:R" '*S-1-5-18:F' | Out-Null
if ($LASTEXITCODE -ne 0) {
  Remove-Item -LiteralPath $tokenFile -Force -ErrorAction SilentlyContinue
  throw "Could not restrict ACLs on the token file (icacls exit $LASTEXITCODE)."
}
Remove-Variable bytes, token
```

Create the launcher. Change `192.168.13.111` in this block if the machine uses another LAN address:

```powershell
$metricsDir = Join-Path $env:LOCALAPPDATA 'Win-CodexBar-Metrics'
$launcher = Join-Path $metricsDir 'start-metrics.ps1'

@'
$ErrorActionPreference = 'Stop'
$metricsDir = Join-Path $env:LOCALAPPDATA 'Win-CodexBar-Metrics'
$tokenFile = Join-Path $metricsDir 'metrics.token'
$exePath = Join-Path $metricsDir 'codexbar-cli.exe'
$env:CODEXBAR_DASHBOARD_TOKEN = (Get-Content -LiteralPath $tokenFile -Raw).Trim()
if ([string]::IsNullOrWhiteSpace($env:CODEXBAR_DASHBOARD_TOKEN)) {
    throw 'The CodexBar metrics token file is empty.'
}

& $exePath serve `
    --host 192.168.13.111 `
    --port 8080 `
    --refresh-interval 300 `
    --identity redacted `
    --metrics `
    --allow-plain-http
exit $LASTEXITCODE
'@ | Set-Content -LiteralPath $launcher -Encoding UTF8
```

Stop any manually started `serve` process already using TCP 8080, then register and start the task. It starts only after this user signs in and does not store the Token in the task action:

```powershell
$taskName = 'Win-CodexBar Prometheus Metrics'
$currentUser = [Security.Principal.WindowsIdentity]::GetCurrent().Name
$metricsDir = Join-Path $env:LOCALAPPDATA 'Win-CodexBar-Metrics'
$launcher = Join-Path $metricsDir 'start-metrics.ps1'
$powershellExe = "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe"
$action = New-ScheduledTaskAction `
  -Execute $powershellExe `
  -Argument ('-NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -File "{0}"' -f $launcher)
$trigger = New-ScheduledTaskTrigger -AtLogOn -User $currentUser
$principal = New-ScheduledTaskPrincipal `
  -UserId $currentUser `
  -LogonType Interactive `
  -RunLevel Limited
$settings = New-ScheduledTaskSettingsSet `
  -AllowStartIfOnBatteries `
  -DontStopIfGoingOnBatteries `
  -ExecutionTimeLimit ([TimeSpan]::Zero) `
  -RestartCount 3 `
  -RestartInterval (New-TimeSpan -Minutes 1)

Register-ScheduledTask `
  -TaskName $taskName `
  -Action $action `
  -Trigger $trigger `
  -Principal $principal `
  -Settings $settings `
  -Description 'Serve authenticated Win-CodexBar Prometheus metrics.' `
  -Force | Out-Null

Start-ScheduledTask -TaskName $taskName
```

Verify the task, listener, and metrics without placing the Token in a child process command line:

```powershell
$taskName = 'Win-CodexBar Prometheus Metrics'
$tokenFile = Join-Path $env:LOCALAPPDATA 'Win-CodexBar-Metrics\metrics.token'

Get-ScheduledTask -TaskName $taskName | Select-Object TaskName, State
Get-ScheduledTaskInfo -TaskName $taskName |
  Select-Object LastRunTime, LastTaskResult, NextRunTime
Get-NetTCPConnection -LocalAddress 192.168.13.111 -LocalPort 8080 -State Listen

$token = (Get-Content -LiteralPath $tokenFile -Raw).Trim()
$response = Invoke-WebRequest `
  -UseBasicParsing `
  -Uri 'http://192.168.13.111:8080/metrics' `
  -Headers @{ Authorization = "Bearer $token" }
$response.StatusCode
$response.Content -split "`n" | Select-String '^codexbar_'
Remove-Variable token
```

To remove the auto-start task:

```powershell
Stop-ScheduledTask -TaskName 'Win-CodexBar Prometheus Metrics' `
  -ErrorAction SilentlyContinue
Unregister-ScheduledTask `
  -TaskName 'Win-CodexBar Prometheus Metrics' `
  -Confirm:$false
```

Store the same Token on the Prometheus host without the `Bearer ` prefix. The following Linux example avoids putting the Token in shell history and limits the file to root and the `prometheus` group:

```bash
sudo install -d -m 0750 -o root -g prometheus /etc/prometheus/secrets
read -rsp 'CodexBar token: ' CODEXBAR_TOKEN; echo
printf '%s' "$CODEXBAR_TOKEN" |
  sudo tee /etc/prometheus/secrets/codexbar.token >/dev/null
unset CODEXBAR_TOKEN
sudo chown root:prometheus /etc/prometheus/secrets/codexbar.token
sudo chmod 0640 /etc/prometheus/secrets/codexbar.token
```

If Prometheus runs under another service group, replace `prometheus` in the directory and `chown` commands with that group.

Add this job to `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: win-codexbar
    scrape_interval: 5m
    scrape_timeout: 4m
    metrics_path: /metrics
    scheme: http
    authorization:
      type: Bearer
      credentials_file: /etc/prometheus/secrets/codexbar.token
    static_configs:
      - targets:
          - 192.168.13.111:8080
```

The first `/metrics` request after the snapshot cache expires builds the snapshot synchronously. Provider collection is followed by Claude account collection and a local cost scan; the first two stages each have bounded fetches, while the cost scan currently has no overall hard timeout. The `5m` interval and `4m` timeout leave substantially more room than a single 75-second provider fetch, but they are operational defaults rather than a guaranteed upper bound. If the first scrape still times out, inspect local log-scan cost and increase both values while keeping `scrape_timeout` below `scrape_interval`.

Validate and reload Prometheus using the method configured for that installation:

```bash
promtool check config /etc/prometheus/prometheus.yml
curl -fsS -G 'http://127.0.0.1:9090/api/v1/query' \
  --data-urlencode 'query=up{job="win-codexbar"}'
```

The query should return `1`. Useful metric families include:

| Metrics | Meaning |
|---------|---------|
| `codexbar_up`, `codexbar_build_info`, `codexbar_snapshot_*`, `codexbar_refresh_interval_seconds` | Export health, build, snapshot generation/age/stale state, staleness threshold, and refresh cadence |
| `codexbar_provider_enabled`, `codexbar_provider_up`, `codexbar_provider_error_code` | Provider enablement and collection state |
| `codexbar_provider_updated_timestamp_seconds`, `codexbar_provider_data_age_seconds` | Provider data update time and data age |
| `codexbar_quota_*{provider,window}` | Used/remaining percentage, reset timestamp, and idle state per quota window |
| `codexbar_cost_today_usd`, `codexbar_cost_last_30_days_usd` | Estimated cost when available |
| `codexbar_account_*` | Per-account state, quota, reset, pace stage/delta, and exhaustion ETA when account snapshots are available |

Optional snapshot fields do not produce a zero-valued placeholder series when the value is unknown. The production snapshot currently does not populate provider service status, separate credits, or account run-out probability, so those schema-reserved metric names do not yield time series. The exporter omits account email, display labels, free-form error messages, credentials, and provider source details from Prometheus labels.

To use the included Grafana dashboard, open **Dashboards → New → Import**, upload [`docs/grafana/codexbar-dashboard.json`](./grafana/codexbar-dashboard.json), and select the Prometheus data source that scrapes this job. The dashboard covers exporter metadata, scrape/snapshot freshness, provider collection and account-adapter state, quota use/remaining/reset/idle values, costs, and per-account pace forecasts currently populated by the production snapshot. Its `job`, `instance`, `provider`, `window`, and `account` variables filter the corresponding panels.

### Config

```powershell
codexbar config providers
codexbar config enable -p cursor
codexbar config disable -p cursor
printf '%s' $env:OPENROUTER_API_KEY | codexbar config set-api-key -p openrouter --stdin
codexbar config validate
```

`enable` / `disable` persist settings. `usage -p <id>` is a one-shot override and does not by itself toggle enabled state the same way.

### Sessions

```powershell
codexbar sessions
codexbar sessions --json --pretty
codexbar sessions --focus '<session-id>'
codexbar sessions --ssh-host user@host
```

### Cache / cookies

Browser cookie import for the app is documented in [COOKIES.md](./COOKIES.md). Prefer Settings → Providers → browser picker on Windows. Manual cookie paste is supported when DPAPI import fails or under WSL.

## Upstream differences (do not copy blindly)

- No Commander/Swift CLI product name `CodexBarCLI`
- No macOS Keychain cookie cache flags as primary docs
- No Homebrew Linux tarball install story as the default Windows path
- Cards / claude-swap–specific CLI behavior from upstream docs may be absent or different — trust `codexbar <cmd> --help` on this binary

## Related

- [CONFIGURATION.md](./CONFIGURATION.md)
- [PROVIDERS.md](./PROVIDERS.md)
- [BUILDING.md](./BUILDING.md)
