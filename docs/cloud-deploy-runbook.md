# Linux API 部署手册（1.2.7）

NexusHub 的 Linux 版本只提供管理 API 和健康检查。它不提供网页、登录页或静态资源，也不会安装 Claude Code、Pi 或 Linux 桌面 App。

部署时从仓库外显式提供 SSH 主机、HTTPS 域名和归档路径。域名、主机、API Key、用户会话和配置不写入 Git。

## 服务与隔离

- 程序：`/usr/local/bin/nexushub-webd`
- systemd 单元：`/etc/systemd/system/nexushub-webd.service`
- 配置：`/etc/nexushub-webd/config.toml` 和 `/etc/nexushub-webd/env`
- 数据：`/var/lib/nexushub-webd/`
- 日志：`/var/log/nexushub-webd/`
- 监听：仅回环地址；反向代理只转发 `api/rpc/` 和 `healthz`，其他路径返回 404

保留 `ProtectSystem=full`、`ProtectHome=read-only`、`NoNewPrivileges=true` 和 `PrivateTmp=true`。Provider 会话根只有在配置存在时才加入精确的 `ReadWritePaths`；新增目录或修改自定义根目录后，需要重新生成 drop-in、执行 `daemon-reload` 并重启服务。

Grok 原生 leader 使用 `/var/lib/nexushub-webd/grok-leader.sock`。凭据和 Provider 配置保持隔离，原生管理操作需要在服务命名空间中可见的工作目录。

## 安装与升级

发布页提供七项资产：macOS ARM64 DMG、校验文件、updater 压缩包及签名、`latest.json`，以及 Linux x86_64 `webd` 服务包和校验文件。`latest.json` 的平台键为 `darwin-aarch64`，文件名中的 `darwin-arm64` 表示同一架构；Linux 服务包不能用于桌面 updater。

使用明确的外部参数部署：

```bash
NEXUSHUB_DOMAIN=panel.example.invalid \
  bash scripts/deploy-cloud.sh SSH_HOST ./dist/nexushub-webd-linux-x86_64.tar.gz
```

升级前先核对校验文件和当前服务状态。1.2.7 首次扫描会为 Claude 新兼容的记录建立基线，不补推历史完成通知；升级后使用新回合验收 Bark。升级脚本会保留业务数据库、Bark 加密材料、通知游标、任务记录和审计数据；原生 Provider 会话和 Codex 数据库不迁移、不改写。

## 管理员 API Key

在服务器生成 Key：

```bash
sudo /usr/local/bin/nexushub-webd admin key-generate --output /absolute/private/api-key
```

输出文件必须不存在，权限为 `0600`。将 Key 输入 App 的“设置 → 远程连接”后立即删除临时文件。服务器只保存摘要，不保存原文。

轮换或撤销：

```bash
sudo /usr/local/bin/nexushub-webd admin key-rotate --output /absolute/private/new-api-key
sudo /usr/local/bin/nexushub-webd admin key-revoke
```

轮换会立即使旧 Key 失效；撤销或未配置 Key 会拒绝业务 RPC。Key 不得出现在 URL、命令参数、日志、浏览器请求或仓库文件中。旧网页 Cookie、登录入口和 Turnstile 配置不再生效。

## 更新与回滚

App 的“更新与维护”页面在本机目标显示“本机 App 更新”，在远程目标显示“腾讯云服务更新”。两者使用各自的更新接口和状态缓存；切换机器不会复用另一台机器的结果。

服务器更新在受保护的 API 服务外启动短时 root unit，写入完成并记录成功后再安排服务重启。若更新失败，先恢复仍保留的服务状态，再检查健康接口。

## 部署后检查

```bash
sudo /usr/local/bin/nexushub-webd --version
sudo systemctl is-active nexushub-webd
sudo systemctl show nexushub-webd -p MainPID -p ProtectHome -p ProtectSystem -p ReadWritePaths
curl -fsS http://127.0.0.1:15742/healthz
```

预期结果：服务为 `active`，健康接口返回成功；未认证业务请求返回 `401`，旧页面和静态资源返回 `404`。

在正式 App 中验证远程连接、会话读取、搜索、附件按需读取、路径复制、Plan 本地保存和用户指令时间线短线。没有原生 Claude、Pi 会话的机器显示空状态，相关管理操作保持禁用。

## 原生数据边界

Claude Code 从 `CLAUDE_CONFIG_DIR/projects` 或用户配置目录读取已有 JSONL，并兼容常见记录字段。未知格式、损坏记录、半写入尾行或活动身份无法确认时保持只读。Grok、Pi 和 Codex 同样只读取当前机器已发现的数据根。

服务关闭不会停止两台机器各自的 monitor。清理只针对本次部署产生的暂存文件；不要删除用户会话、凭据、Keychain 项或与 NexusHub 无关的服务文件。
