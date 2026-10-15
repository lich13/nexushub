# Linux API 部署手册

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

升级前先核对校验文件和当前服务状态。远程 API 协议为 2，必须先升级服务，再安装新版 App。协议不兼容时会提示更新并拒绝远程操作，本机仍可使用。升级会清除 NexusHub 自有的 Pi 提供方、流、投递和 Pi 专属事件，以及三项 notify_pi 设置和 Pi 路径变量；其余业务数据库内容、Bark 加密材料、通知游标、任务记录和审计数据保留；原生 Provider 会话和 Codex 数据库不迁移、不改写。

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

App 的“更新与维护”页面在本机目标显示“本机 App 更新”，在远程目标显示“远程服务更新”。两者使用各自的更新接口和状态缓存；切换机器不会复用另一台机器的结果。

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

## 上下文与时间线兼容

1.2.21 的可选 contextBlocks、timeline 和较新页游标仍使用 API 协议 2；旧客户端忽略新增字段，新客户端连接旧服务时本地整理原始包装并保留逐页加载。先升级 API 才能在远程使用完整时间线索引和有界跳转。升级不改写原生记录，也不新增客户端文件路径权限。

## 升级验收

部署或安装时只核对对应提交、资产校验、服务版本、健康状态和 App 启动。以下功能、交互与真实入口项目由用户验收；助手仅在明确要求或定位已证实的硬性故障时执行必要部分。未执行项标记“待用户验收”，不把 fixture 或服务启动视为功能通过。

Grok 误保护与隐藏扫描的验收入口：在 Grok 为原先误保护的闲置线程打开删除预览，应允许查看候选；仍被原生程序打开的会话应说明保护原因。在“更新与维护”扫描隐藏线程，检查进度、每页最多 100 项、停止后保留结果但不能清理，以及重新扫描。旧 API 缺少 `hidden_cleanup_scan` 时保留单次扫描；先升级 API 再安装 App 才能使用两端分批扫描。

- 在包含环境、AGENTS.md 和附件包装的主线程及子线程中，默认应只看到附件、真实请求及一行“上下文”；展开可读原文。搜索真实请求或 AGENTS.md 应定位并展开，自动环境字段不应产生消息命中。
- 打开长线程并悬停、点击最早时间线刻度，检查窄预览、较早/较新分页和返回最新；大量刻度可用滚动及 Home/End 到达。检查机器切换后迟到结果不会出现在新线程。
- 在正式 App 的本机与远程目标验证列表、设置版本、详情、搜索、按需附件、Plan 本地保存、路径与用户指令时间线。网络或钥匙串失败必须就地显示，不能静默切回本机；HTTPS 成功不能替代正式 App 验收。
- 用长 Codex、Claude、Grok 和子智能体样本检查初始最新定位、连续上滚、加载中继续滚动、工具展开和机器切换；轮询不能丢失历史或重置父线程位置。父线程不追加时仍检查直属子智能体的当前状态。
- 三种 Provider 按 100MiB、300MiB 字节阈值分色；在仓库外独立只读核对实际字节与统计范围。大小统计无需开放工作目录或附件根。旧服务缺字段显示 —，部分结果保留 ≥，不能冒充完整大小。
- `system.capabilities` 保持协议 2，子智能体能力使用 `thread_subagents`。详情及附件只接受原生身份、主线程上下文和有界游标，不能接受任意路径。缺少真实样本时只记录空状态、接口或 fixture 覆盖。
- 用隔离数据检查预览绑定的改名、归档、删除和混合清理，确认只执行预览 ID 与指纹。保留 API Key、Keychain、Bark、通知状态和 systemd 隔离；不使用用户会话进行破坏性验收。
- 从本机与远程分别发起新的“NexusHub 推送测试”，等待真实作业终态，并核对设备实收。HTTP 与 Bark JSON 同时成功仅表示服务端受理。关闭 App 后 monitor 应继续运行且无额外监听端口。

钥匙串读取后台串行，15 秒等待超时仍保护已开始的原生操作；保存与移除连接须完成事务或回滚。若系统要求授权，只在系统对话框处理。

## 迁移与通知保护

迁移只清理 NexusHub 自有的旧 web、Goal、Pi 与 Gotify 配置、Token、队列和专属记录；保留不可执行的兼容 tombstone。SQLite 遗留页与 WAL 清理可在中断后继续。不得删除原生 Provider 程序、会话或其他应用数据库。

Bark 保留成功记录、游标和启用基线，不补发历史失败或未知事件。每分段最多 3 次，重试等待 15／60 秒（±20%），首次投递起最多 10 分钟并尊重 Retry-After；结果未知仍可能重复提醒。故障与重启验证使用隔离 HTTP 服务，不能破坏生产网络。

monitor 日志目标使用 `nexushub_webd`，只输出安全分类、状态码、耗时、次数和匿名标识。外部退休服务的卸载属于独立运维操作，需要明确归属；不修改共用 Nginx、TLS 或其他服务。

验收结束关闭本轮预览和转发，仅清理确认归属的下载、测试和部署暂存；保留正式服务、用户数据、连接、凭据与共享缓存。性能收益与实际回收空间分别计量。
