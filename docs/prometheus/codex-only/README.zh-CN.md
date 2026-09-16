# Codex-only Prometheus 监控包

本目录只监控 Win-CodexBar 中的 Codex provider，包含 Windows 启动脚本、Prometheus 抓取示例、告警规则和可直接导入的 Grafana Dashboard。

## 1. Windows 节点

把 `windows` 目录复制到 Windows 节点，然后依次运行：

```cmd
cd /d C:\path\to\windows
setup-codex-only.cmd
start-codex-metrics.cmd
```

请用已经执行过 `codex login` 的同一个普通 Windows 用户运行这三个 CMD；Codex OAuth、CodexBar 配置和缓存都位于当前用户目录。只有后面的 Windows 防火墙命令需要管理员 PowerShell。

`setup-codex-only.cmd` 会完成以下操作：

- 下载固定版本的 `codexbar-cli.exe.sha256` 并严格解析哈希；本地 EXE 不匹配时才下载同版本 EXE，校验通过后再替换。
- 在共享 CodexBar 配置中只保留 Codex provider；桌面版也会读取这份配置。
- 生成 64 位十六进制 Bearer Token，并限制 Token 文件 ACL。

启动脚本默认绑定 `192.168.13.111:8081`。每台节点必须改成自己的局域网 IP：

```cmd
set CODEXBAR_BIND_IP=192.168.13.111
set CODEXBAR_PORT=8081
start-codex-metrics.cmd
```

另开一个 CMD 验证端点和 provider 标签：

```cmd
set CODEXBAR_BIND_IP=192.168.13.111
set CODEXBAR_PORT=8081
test-codex-metrics.cmd
```

测试脚本会要求 `codexbar_up` 为 1、Codex 已启用、`codexbar_reset_credits_available{provider="codex"}` 存在且值为 `-1` 或非负整数，并拒绝任何非 `codex` 的 provider 指标。启动后如果桌面端修改了共享 provider 配置，请重新运行测试脚本；Prometheus 示例中的 `metric_relabel_configs` 仍会丢弃所有非 Codex provider 序列。

在管理员 PowerShell 中只允许 Prometheus 服务器访问端口：

```powershell
New-NetFirewallRule `
  -DisplayName 'Win-CodexBar Codex metrics' `
  -Direction Inbound `
  -Action Allow `
  -Protocol TCP `
  -LocalAddress 192.168.13.111 `
  -LocalPort 8081 `
  -RemoteAddress '<PROMETHEUS_SERVER_IP>' `
  -Profile Domain,Private
```

该命令只新增一条限定来源地址的 Allow 规则。要确保端口确实“只允许 Prometheus”，还需确认当前 Domain/Private 配置文件的默认入站动作为 Block，并且没有其他覆盖 TCP 8081 或该程序的更宽泛入站 Allow 规则。

## 2. Prometheus

示例中的 3 台节点共用一个 Token。当前三台节点使用同一个启动 Token 时，可直接使用该配置。

在全新节点上使用本包时，先在第一台运行 `setup-codex-only.cmd` 生成 `metrics.token`；再把这同一个文件安全复制到另外两台的 `windows` 目录，然后在另外两台运行 `setup-codex-only.cmd`。脚本会保留已有 Token 并重新设置本机 ACL。最后将同一 Token 的内容复制到 Prometheus 主机，文件中只写 Token，不带 `Bearer ` 前缀：

```bash
sudo install -d -m 0750 -o root -g prometheus /etc/prometheus/secrets
sudo install -d -m 0750 -o root -g prometheus /etc/prometheus/rules
read -rsp 'CodexBar token: ' CODEXBAR_TOKEN; echo
printf '%s' "$CODEXBAR_TOKEN" |
  sudo tee /etc/prometheus/secrets/codexbar.token >/dev/null
unset CODEXBAR_TOKEN
sudo chown root:prometheus /etc/prometheus/secrets/codexbar.token
sudo chmod 0640 /etc/prometheus/secrets/codexbar.token
sudo install -m 0644 codexbar-codex-alerts.yml /etc/prometheus/rules/codexbar-codex-alerts.yml
```

输入 Token 后按回车。把 `prometheus-codexbar-example.yml` 中的 `rule_files` 和 `scrape_configs` 合并到现有 `prometheus.yml`。`job_name` 可以修改，但必须保留每个 target 的 `monitor: codexbar-codex` 标签，告警和 Dashboard 使用该标签识别监控目标。

`authorization` 对整个 scrape job 生效。如果每台节点的 Token 不同，请拆成 3 个 job，每个 job 配置自己的 `credentials_file` 和一个 target，并在每个 target 上保留相同的 `monitor: codexbar-codex` 标签。完成后检查配置：

```bash
promtool check rules /etc/prometheus/rules/codexbar-codex-alerts.yml
promtool check config /etc/prometheus/prometheus.yml
```

重载 Prometheus 后查询：

```promql
up{monitor="codexbar-codex"}
codexbar_provider_up{monitor="codexbar-codex",provider="codex"}
codexbar_quota_remaining_percent{monitor="codexbar-codex",provider="codex"}
codexbar_reset_credits_available{monitor="codexbar-codex",provider="codex"}
codexbar_reset_credits_next_expiry_timestamp_seconds{monitor="codexbar-codex",provider="codex"}
```

`codexbar_reset_credits_available` 的值为非负整数时，表示当前可用重置卡张数，其中 `0` 表示已经用完；值为 `-1` 时，表示 PAT 模式、接口不支持或本次查询失败，不能当作真实张数参与 `min()`、`avg()` 等聚合。需要统计张数时先用 `codexbar_reset_credits_available >= 0` 过滤未知值。

`codexbar_reset_credits_next_expiry_timestamp_seconds` 表示当前可用重置卡中最早到期的一张，值为 Unix 秒时间戳。没有可用卡、接口未提供有效期或重置卡数据无法获取时不输出该指标；它不会暴露卡 ID，也不会为每张卡创建标签。

Win-CodexBar 会把 `/wham/rate-limit-reset-credits` 的查询结果缓存 10 分钟，成功结果和“无法获取”状态都会缓存。在同一账号和认证范围内，常规刷新即使比 Prometheus 抓取更频繁，重置卡接口通常也只会每 10 分钟重新查询一次。检测到可疑的周额度重置时会绕过常规缓存取得独立库存观察，用于避免把旧缓存当成消费重置卡的证据。

告警阈值为：重置卡数量等于 `0` 持续 5 分钟时 warning；采集数据新鲜但重置卡指标缺失或值小于 `0` 持续 10 分钟时单独 warning；剩余 `5% < quota <= 20%` 时 warning，剩余 `quota <= 5%` 时 critical。采集正常但连续 15 分钟没有任何配额窗口时也会 warning，防止接口字段变化造成静默失明。规则覆盖 Codex 的 session、weekly、model、tertiary 和附加动态窗口；如只关注主订阅窗口，可在额度规则中增加 `window=~"session|weekly"`。

## 3. Grafana

导入 [`../../grafana/codexbar-codex-dashboard.json`](../../grafana/codexbar-codex-dashboard.json)：

1. 打开 **Dashboards -> New -> Import**。
2. 上传 JSON 文件。
3. 在 `Prometheus` 变量中选择数据源。
4. 面板界面已中文化；“采集任务”和“节点”默认选择“全部”，“额度窗口”默认选择 `weekly`，也可以切换为其他窗口或“全部”。示例 job 名为 `codex-bar`，也可以使用其他名称。

Dashboard 固定使用 `provider="codex"`，不会展示其他 provider。当前值和历史图统一展示剩余额度，“Codex 重置卡”面板同时显示可用张数和最近到期时间，并把张数 `-1` 映射为“无法获取”。它不包含多账号面板，因为 Codex 当前不生成对应指标。底部“辅助信息”默认折叠，展开后可同时查看 Codex 本地估算成本、导出器元数据和 Codex 采集详情；成本来自本地会话日志估算，不是订阅账单。

初始化与启动脚本会确保 `/metrics` 启动时只有 Codex provider；Prometheus 的抓取过滤、告警规则和 Dashboard 则持续只使用 Codex 数据。相同 HTTP 服务上的 `/usage`、`/cost`、`/dashboard/v1/snapshot` 等数据路由受同一个 Bearer Token 保护；`/health` 只公开版本和状态。防火墙仍应只允许 Prometheus 服务器访问。
