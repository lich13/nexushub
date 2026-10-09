# Linux API 部署手册（1.2.15）

NexusHub 的 Linux 版本只提供管理 API 和健康检查。它不提供网页、登录页或静态资源，也不会安装 Claude Code 或 Linux 桌面 App。

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

升级前先核对校验文件和当前服务状态。1.2.8 将远程 API 协议升为 2，必须先升级服务，再安装新版 App。协议不兼容时会提示更新并拒绝远程操作，本机仍可使用。升级会清除 NexusHub 自有的 Pi 提供方、流、投递和 Pi 专属事件，以及三项 notify_pi 设置和 Pi 路径变量；其余业务数据库内容、Bark 加密材料、通知游标、任务记录和审计数据保留；原生 Provider 会话和 Codex 数据库不迁移、不改写。

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

在正式 App 中验证远程连接、会话读取、搜索、附件按需读取、路径复制、Plan 本地保存和用户指令时间线短线。没有原生 Claude 会话的机器显示空状态，相关管理操作保持禁用。

## 原生数据边界

Claude Code 从 `CLAUDE_CONFIG_DIR/projects` 或用户配置目录读取已有 JSONL，并兼容常见记录字段。未知格式、损坏记录、半写入尾行或活动身份无法确认时保持只读。Grok 和 Codex 同样只读取当前机器已发现的数据根。

关闭桌面 App 后两台机器继续独立监测；停止远程服务会同时停止该服务内的远程监测。清理只针对本次部署产生的暂存文件；不要删除用户会话、凭据、Keychain 项或与 NexusHub 无关的服务文件。

## 1.2.8 验收要点

`system.providers` 只报告 Codex、Claude Code 和 Grok；`pi.*` 返回不可用。检查生成的 systemd 会话写入白名单不再包含 Pi，保留现有隔离参数与其他项目配置。不得删除 Pi 原生会话、程序或工作目录。

用专用数据验证普通/归档 Codex 删除及隐藏候选混合清理，确认执行只覆盖预览 ID 和指纹。用户数据不用于破坏性验收。核对 API Key、Keychain 连接和其他 Provider Bark 设置仍可用。恢复材料仅保留到正式验收完成，之后精确删除本轮暂存。

## 1.2.9 子智能体验收

API 协议保持 2；先升级服务，再安装 App。已认证的 `system.capabilities` 应包含 `thread_subagents: true`。`threads.subagentDetail` 只接受主线程与子线程身份和分页参数，不接受路径；子线程附件必须同时提供原主线程上下文。API Key、Keychain、Bark、通知游标及隔离配置沿用现有值。

正式 App 在当前机器的 Codex 父线程中打开子智能体活动，验证详情、嵌套返回、较早消息、附件与切换机器后的隔离。无真实父子记录时只验证空状态、拒绝路径和隔离夹具，并明确记录此边界；不能把夹具成功当作真实会话验收。关闭 App 后既有后台监测仍须运行。

## 1.2.10 远程连接验收

本补丁保持 API 协议 2，保留现有 Keychain、服务器 Key 摘要和通知配置。升级正式 App 后，在现有连接中检查远程列表、设置版本与详情读取，再切回本机；网络或钥匙串失败必须显示当前目标的错误，不能静默回退。服务器 HTTPS 请求成功不能替代 App 界面验收。

钥匙串读取在后台串行执行并提供 15 秒等待反馈；保存或移除连接的原生事务会等待完成或回滚。若 macOS 要求授权，只在系统对话框处理，不把 Key 写入命令、日志或临时报告。

## 1.2.11 升级验收

先更新 API 服务，再安装正式 App；协议仍为 2，无配置或数据库迁移。保留现有 API Key、Keychain 连接、Bark、monitor 和原生会话。

从 App 切换远程目标，核对子智能体历史开始记录、统一任务名和详情直属汇总；观察开始到完成的数量变化，父线程不追加消息时也应更新。嵌套详情只统计当前子线程的直属下级。旧服务缺少汇总时应提示升级，不将缺失数据当作零。

本机与远程使用同一内部页面标记过滤规则。远程没有原生子线程时，可用隔离会话验证接口、关联和机器隔离，但须明确记录夹具边界，不宣称真实子线程验收。验收后删除本轮隔离数据和部署暂存，保留正式服务及用户数据。

## 1.2.12 升级验收

本版仅精简前端说明，API 仍为协议 2，服务与客户端版本同步升级。核对子智能体汇总和清理面板不再显示重复描述，实际状态、预览和处理结果可正常读取；不执行真实用户数据清理来验证文案。

## 1.2.14 通知退役迁移

先升级 API 服务，再安装 App；协议保持 2。升级清理 NexusHub 自有的 Gotify 配置、加密 Token、投递记录和测试任务，保留 Bark 凭据、基线及去重记录。迁移中断后会继续清理 SQLite 遗留页和 WAL；不会读取或修改其他应用数据库。

已部署的外部 Gotify 服务由运维仓库按显式参数卸载：先关闭投递，核对归属后移除专属容器、未共享镜像、反代 include、账号、Token 和持久化目录；不归档退休凭据。只修改该服务的反代位置，保留现有 API、TLS、共用 Nginx 与其他项目。

验收旧服务入口不可用，NexusHub 健康检查和 API 鉴权正常，正式 monitor 继续运行；同时核对 Bark 设置与 Keychain 连接保持。只清理本轮明确归属的暂存。

## 1.2.15 Bark 重试升级

先升级远程 API，再安装 App；API 协议保持 2。升级自动扩展 NexusHub 投递表，保留 Bark 密钥、成功记录、游标及基线，不补发历史失败或未知事件。旧程序配置导入入口不可执行，原有正式配置继续使用。

核对自有 monitor 的日志过滤目标为 `nexushub_webd`；旧默认值 `nexushub-webd=info` 无法匹配 Rust 日志目标。日志只能包含安全分类、状态码、耗时、次数和匿名标识。不要打印配置、地址、Key、通知正文或原始网络异常。

分别从本机与远程 API 发起新的“NexusHub 推送测试”，确认作业从运行进入真实终态，并由设备确认对应机器的通知。关闭 App 后验证 monitor 继续运行且没有监听端口。故障与重启重试使用隔离 HTTP 服务验证，不破坏生产网络；HTTP code 与 Bark JSON code 同时成功仅证明服务端受理。

每分段最多 3 次，两次重试等待 15/60 秒（±20%），首次投递起最多 10 分钟；尊重 Retry-After。未知结果也可能重试，不能承诺绝不重复。凭据或端点变更会停止旧队列；用户可重新发起独立测试。验收完成后仅清理本轮明确归属的暂存与必要恢复材料。
