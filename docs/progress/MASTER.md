# NexusHub 当前状态（维护者）

## 当前版本

当前维护源码版本为 1.3.9，核心提交为 [`f150ce8`](https://github.com/lich13/nexushub/commit/f150ce8970988ed843ffe92fe1421029f6f0db9b)，基线为 `a8740b3 / v1.3.8`。本轮修复 Grok／Claude 删除索引时的 systemd 写权限，避免无匹配记录时重写共享文件；Claude 自动内容归入默认折叠活动，Hook 卡片统一状态与操作样式。API 协议仍为 2；macOS App 是唯一界面。1.3.9 发布、部署与安装记录将在正式交付后补齐。

历史实现与交付由 Git 和 [Release](https://github.com/lich13/nexushub/releases) 保留。完整回归、跨平台构建和既有性能检查以发布提交对应的 [CI](https://github.com/lich13/nexushub/actions/workflows/ci.yml) 为准。

## 已实现范围

| 工作块 | 当前行为 | 核心提交 |
|---|---|---|
| 多选与菜单 | 三种 Provider 共用 ⌘／Shift／普通／键盘点击和锚点；删除无数量上限，归档／恢复按 100 项分批，轮询不新增选择，切换范围清空。 | `5ffd995` |
| Codex 删除 | 按确认的原生父子关系直接删除普通／归档 rollout、压缩副本、writer lock、shell snapshots、visualizations、数据库、分页历史和名称索引；循环去重，错误逐项返回。 | `5ffd995` |
| Claude 管理 | 改名保留原有身份与占用规则；删除按项目相对路径和文件名定位主文件、同名目录、旁路及共享索引，不依赖正文完整性。 | `5ffd995` |
| Claude 自动内容 | 完整命令包装显示为命令与原始参数；通知、回显、上下文和摘要不创建用户回合，在原位置默认折叠并保留搜索、分页锚点。 | `f150ce8` |
| Claude／Grok 读取 | Claude 空文件、命名记录和仅元数据会话显示“暂无对话”；Grok 缺少 `updates.jsonl` 时读取 `chat_history.jsonl`，过滤系统提示、system_reminder 和加密推理。 | `5ffd995` |
| 直接删除边界 | fingerprint 可省略且不参与删除，expectedCount 不参与清理；缺失文件完成，权限／数据库失败继续处理其他目标并返回 residuals。 | `5ffd995` |
| Playwright 浏览器守卫 | 移除品牌 channel，启动探针核对 Playwright 缓存内真实可执行文件；浏览器回归与 macOS 有头性能检查交给三项 CI。 | `0d7afea` |
| Bark 与 hooks | 新事件统一单条短摘要；本机 hooks 须明确安装，远端继续轮询。Claude prompt 身份核验后每回合每类认领一次，保留有效旧完成以补发，完成后不单独提醒空闲。Codex 保留逐问题身份和提醒。 | `01be4b4` |

实现、安全边界、旧字段与旧队列兼容见[架构说明](../ARCHITECTURE.md)。删除按确认目标逐项执行，失败返回 residuals 并继续其他目标；归档、恢复和改名仍沿用各自规则。Claude 删除不触碰工作目录或共享文件中的非目标记录。

## 必要验证

以下证据覆盖 1.3.9 核心实现至 `f150ce8`；完整回归、跨平台构建和正式发布门禁以最终版本提交对应的 GitHub Actions 为准。

| 范围 | 已通过证据 |
|---|---|
| Rust core | 共享索引命中／未命中写入、Claude 命令还原、自动上下文不生成回合、工具状态连续、压缩摘要搜索及既有 Provider 删除测试通过；core/webd Clippy 通过。 |
| Rust webd | 受影响 API 与适配测试通过；三项主 CI 继续覆盖完整 workspace、签名安装安全和 Linux 服务包。 |
| Tauri | 受影响设置、入口守卫、探针命令和 Clippy 测试通过；hooks 入口只限本机，不开放 Linux RPC 或远端配置写入。 |
| WebUI | Vitest、类型检查通过；覆盖 Claude 自动内容折叠、搜索展开和分页展开状态；Playwright 浏览器测试仅由 GitHub Actions 执行。 |
| 静态与脚本 | Rust 格式、workspace Clippy、Rust／TypeScript 契约、安装／systemd 路径测试通过；腾讯云合成命名空间复现旧只读错误并验证新规则下索引原子替换与配置只读。 |

未使用真实用户会话执行删除测试，未发送真实 Bark 验收消息；功能、视觉和端到端验收由用户完成。完整跨平台和性能门禁继续由现有三项 CI 执行，没有新增性能门禁或本机正式打包。

核心收尾已覆盖删除预览的整树能力、includeDescendants 固定目标、空会话读取、原生 Grok 历史、超过 100 项、超大／深层目录、循环父子关系、旁路权限错误和共享索引保留。版本／工作区、契约、格式、Clippy、安装脚本和隐私检查通过；浏览器与完整跨平台回归由 GitHub Actions 执行。

## 正式交付

1.3.8 已同步现有 manifest 和两份 lock 中的自身版本，发布说明使用中文六节，保留八项正式资产：macOS DMG／校验、updater／签名、latest.json，以及 Linux API 服务包／校验／签名。核心修复提交 [`c645e44`](https://github.com/lich13/nexushub/commit/c645e44885a43f34ebddefb3e3f172c84a38a88b) 的[三项主 CI](https://github.com/lich13/nexushub/actions/runs/39840632543)（macOS WebKit 性能作业重跑后）全绿；[`v1.3.8` Release CI](https://github.com/lich13/nexushub/actions/runs/39843284210) 全绿并发布[八项正式资产](https://github.com/lich13/nexushub/releases/tag/v1.3.8)。已使用正式 Linux 服务包部署远端 API，版本为 1.3.8、service 为 active、health 为 `{"ok":true}`；已使用正式 macOS ARM64 DMG 原地安装 App，bundle 版本为 1.3.8 且启动成功，没有保留旧副本。收尾未创建预览服务或 SSH 转发；两个真实样例未执行删除，保留待用户验收。

上一版交付记录：提交 [`51dfcd5`](https://github.com/lich13/nexushub/commit/51dfcd5225eef215b4dade342d4330d3ad42159f) 的 [三项主 CI](https://github.com/lich13/nexushub/actions/runs/39585391199) 全绿，`v1.3.5` 的 [Release CI](https://github.com/lich13/nexushub/actions/runs/39586479754) 全绿并发布 [八项正式资产](https://github.com/lich13/nexushub/releases/tag/v1.3.5)。已使用正式 Linux 资产升级远端 API，服务版本、健康、公开 healthz、未授权 API 和退役路径检查通过；已安装正式 macOS ARM64 App，版本为 1.3.5 且启动成功。部署和安装保留既有回退材料，未自动启用 Claude hooks。

1.3.6 核心实现由 [`01c5695`](https://github.com/lich13/nexushub/commit/01c5695)、[`2aaa929`](https://github.com/lich13/nexushub/commit/2aaa929) 和 [`1cfa0a7`](https://github.com/lich13/nexushub/commit/1cfa0a7) 完成；版本、文档和 CI 启动器收尾提交为 [`7254912`](https://github.com/lich13/nexushub/commit/7254912)。该提交对应的[三项主 CI](https://github.com/lich13/nexushub/actions/runs/39716648742)（macOS 性能检查重跑后）与 [`v1.3.6` Release CI](https://github.com/lich13/nexushub/actions/runs/39717900484) 全绿，已发布[八项正式资产](https://github.com/lich13/nexushub/releases/tag/v1.3.6)。已使用正式 Linux 资产升级远端 API，服务版本、健康、公开 healthz、未授权 API 和退役路径检查通过；已安装正式 macOS ARM64 App，版本为 1.3.6 且启动成功，并保留 1.3.5 回退 App。收尾清理仅移除本轮临时文件和 NexusHub 专属缓存，实际回收 22,524 KiB；未自动启用 Claude hooks。

1.3.7 核心实现由 [`6ca0716`](https://github.com/lich13/nexushub/commit/6ca0716)、[`e363069`](https://github.com/lich13/nexushub/commit/e363069)、[`d73b2d4`](https://github.com/lich13/nexushub/commit/d73b2d4)、[`0d7afea`](https://github.com/lich13/nexushub/commit/0d7afea) 和 [`70602df`](https://github.com/lich13/nexushub/commit/70602df) 完成；版本、文档和发布说明收尾提交为 [`cffb3e9`](https://github.com/lich13/nexushub/commit/cffb3e9)。该提交对应的[三项主 CI](https://github.com/lich13/nexushub/actions/runs/39727312472) 与 [`v1.3.7` Release CI](https://github.com/lich13/nexushub/actions/runs/39728000909) 全绿，已发布[八项正式资产](https://github.com/lich13/nexushub/releases/tag/v1.3.7)。已使用正式 Linux 服务包升级远端 API，服务版本为 1.3.7、systemd 为 active、回环 healthz 成功；已安装正式 macOS ARM64 App，bundle 版本为 1.3.7 且启动成功，并保留 1.3.6 回退 App。安装和部署临时文件已清理，未自动启用 Claude hooks。

## 待用户验收

在三个 Provider 中测试 ⌘／Shift 多选、复选框及菜单；用专门测试线程检查空线程、待回复、回退 rollout 和整树删除，确认 Claude 运行时仍能管理同目录的其他历史会话。

随后在本机 Probe 设置中确认安装 Claude hooks，验证完成、问题、授权和失败提醒，完成后应无空闲重复提醒；Codex 同回合多个独立问题仍应分别通知。手机实收与日常交互未由助手代验。

1.3.7 真实验收：关闭占用目标的 codex 写入者后，预览并删除线程 `01a16116`，确认主 rollout、writer lock、shell snapshot 和 visualization 旁路均消失；验证原冲突 Claude 会话仍可改名和删除。浏览器回归与 macOS 有头性能检查以 GitHub Actions 为准，确认验收过程不需要或启动系统 Google Chrome。
