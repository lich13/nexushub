# NexusHub 当前状态（维护者）

## 当前版本

当前维护源码版本为 1.3.10，核心提交为 [`8e222e1`](https://github.com/lich13/nexushub/commit/8e222e1612199f5d23bfd8ffb67793918edefb83)，默认 DSH 路径修复为 [`d5f9564`](https://github.com/lich13/nexushub/commit/d5f9564544655df9a6247c681eab2eed984c6e24)，版本收尾提交为 [`54f40dd`](https://github.com/lich13/nexushub/commit/54f40dd88fef63b5f3dcd5661163d97e281ebcae)，基线为 `d13e4d6 / v1.3.9`。本轮修复 Claude 旧失败提醒的首次发送与重试校验，新增本机及远端 DSH 归档清理。API 协议仍为 2；macOS App 是唯一界面。

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
| Claude 提醒复核 | 首次处理、首次发送和每次重试均复核当前主回合与实际执行活动；新回合、恢复执行、成功及取消终结旧失败提醒，保留有效旧完成补发。 | `8e222e1` |
| DSH 归档清理 | 当前机器的维护页预览归档及已核验 subagent 后代；退出 DSH 后按固定身份清理专属文件和匹配索引，支持压缩文件、自定义根、并发保护与逐项重试。 | `8e222e1` |

实现、安全边界、旧字段与旧队列兼容见[架构说明](../ARCHITECTURE.md)。删除按确认目标逐项执行，失败返回 residuals 并继续其他目标；归档、恢复和改名仍沿用各自规则。Claude 删除不触碰工作目录或共享文件中的非目标记录。

## 必要验证

以下证据覆盖 1.3.10 核心实现及收尾；版本提交的[三项主 CI](https://github.com/lich13/nexushub/actions/runs/40030435588) 已通过（前端 Ubuntu 依赖下载超时后重跑该作业）。后续提交与正式发布门禁以对应的 GitHub Actions 为准。

| 范围 | 已通过证据 |
|---|---|
| Rust core | Claude 真实失败、新回合、恢复执行、取消、历史完成补发及辅助元数据测试通过；DSH 归档隔离、子线程与 fork、压缩文件、缺失文件、恢复、并发修改、部分失败与再次清理测试通过。 |
| Rust webd | 实际 Cargo CLI 覆盖 Hook／轮询去重、首发前恢复、重试前恢复及旧完成补发；受影响 API、适配器和完整 workspace 测试由主 CI 通过。 |
| Tauri | 新增 DSH 薄入口、命令注册及远端 allowlist 契约通过；Tauri 测试和 Clippy 通过。 |
| WebUI | Vitest 811 项及类型检查通过，覆盖 DSH 固定确认身份、失败结果、缺少数据、旧服务能力和机器切换后的状态重置；浏览器回归与 macOS 性能检查由主 CI 通过。 |
| 静态与脚本 | Rust 格式、两端 Clippy、Rust／TypeScript 契约、版本／工作区、安装脚本、systemd 路径及隐私检查通过。 |

未使用真实用户会话执行删除测试，未发送真实 Bark 验收消息；功能、视觉和端到端验收由用户完成。完整跨平台和性能门禁继续由现有三项 CI 执行，没有新增性能门禁或本机正式打包。

DSH 使用 workspace 原生归档名单，只纳入可核验的 subagent 后代，独立 fork 保留。执行请求不接受路径，恢复或归属变化跳过；共享索引仅原子移除成功目标，无匹配不写入。自定义持久化后端和无法识别格式明确拒绝，缺少数据目录显示空状态。

## 正式交付

1.3.10 的发布、远程服务器部署和 Mac 原地安装尚待最终提交的 CI 与 Release CI 通过后完成。

1.3.9 的[三项主 CI](https://github.com/lich13/nexushub/actions/runs/39865303352)与 [`v1.3.9` Release CI](https://github.com/lich13/nexushub/actions/runs/39866444443) 全绿，已发布[八项正式资产](https://github.com/lich13/nexushub/releases/tag/v1.3.9)。已使用正式 Linux 服务包部署远端 API，版本为 1.3.9、service 为 active、health 为 `{"ok":true}`；已用 `NexusHub-1.3.9-darwin-arm64.dmg` 原地安装 macOS App，校验文件和签名通过，bundle 版本为 1.3.9 且启动成功，没有保留旧副本。安装与部署脚本已清理各自临时文件；真实用户会话未执行删除，仍待用户验收。

1.3.8 已同步现有 manifest 和两份 lock 中的自身版本，发布说明使用中文六节，保留八项正式资产：macOS DMG／校验、updater／签名、latest.json，以及 Linux API 服务包／校验／签名。核心修复提交 [`c645e44`](https://github.com/lich13/nexushub/commit/c645e44885a43f34ebddefb3e3f172c84a38a88b) 的[三项主 CI](https://github.com/lich13/nexushub/actions/runs/39840632543)（macOS WebKit 性能作业重跑后）全绿；[`v1.3.8` Release CI](https://github.com/lich13/nexushub/actions/runs/39843284210) 全绿并发布[八项正式资产](https://github.com/lich13/nexushub/releases/tag/v1.3.8)。已使用正式 Linux 服务包部署远端 API，版本为 1.3.8、service 为 active、health 为 `{"ok":true}`；已使用正式 macOS ARM64 DMG 原地安装 App，bundle 版本为 1.3.8 且启动成功，没有保留旧副本。收尾未创建预览服务或 SSH 转发；两个真实样例未执行删除，保留待用户验收。

上一版交付记录：提交 [`51dfcd5`](https://github.com/lich13/nexushub/commit/51dfcd5225eef215b4dade342d4330d3ad42159f) 的 [三项主 CI](https://github.com/lich13/nexushub/actions/runs/39585391199) 全绿，`v1.3.5` 的 [Release CI](https://github.com/lich13/nexushub/actions/runs/39586479754) 全绿并发布 [八项正式资产](https://github.com/lich13/nexushub/releases/tag/v1.3.5)。已使用正式 Linux 资产升级远端 API，服务版本、健康、公开 healthz、未授权 API 和退役路径检查通过；已安装正式 macOS ARM64 App，版本为 1.3.5 且启动成功。部署和安装保留既有回退材料，未自动启用 Claude hooks。

1.3.6 核心实现由 [`01c5695`](https://github.com/lich13/nexushub/commit/01c5695)、[`2aaa929`](https://github.com/lich13/nexushub/commit/2aaa929) 和 [`1cfa0a7`](https://github.com/lich13/nexushub/commit/1cfa0a7) 完成；版本、文档和 CI 启动器收尾提交为 [`7254912`](https://github.com/lich13/nexushub/commit/7254912)。该提交对应的[三项主 CI](https://github.com/lich13/nexushub/actions/runs/39716648742)（macOS 性能检查重跑后）与 [`v1.3.6` Release CI](https://github.com/lich13/nexushub/actions/runs/39717900484) 全绿，已发布[八项正式资产](https://github.com/lich13/nexushub/releases/tag/v1.3.6)。已使用正式 Linux 资产升级远端 API，服务版本、健康、公开 healthz、未授权 API 和退役路径检查通过；已安装正式 macOS ARM64 App，版本为 1.3.6 且启动成功，并保留 1.3.5 回退 App。收尾清理仅移除本轮临时文件和 NexusHub 专属缓存，实际回收 22,524 KiB；未自动启用 Claude hooks。

1.3.7 核心实现由 [`6ca0716`](https://github.com/lich13/nexushub/commit/6ca0716)、[`e363069`](https://github.com/lich13/nexushub/commit/e363069)、[`d73b2d4`](https://github.com/lich13/nexushub/commit/d73b2d4)、[`0d7afea`](https://github.com/lich13/nexushub/commit/0d7afea) 和 [`70602df`](https://github.com/lich13/nexushub/commit/70602df) 完成；版本、文档和发布说明收尾提交为 [`cffb3e9`](https://github.com/lich13/nexushub/commit/cffb3e9)。该提交对应的[三项主 CI](https://github.com/lich13/nexushub/actions/runs/39727312472) 与 [`v1.3.7` Release CI](https://github.com/lich13/nexushub/actions/runs/39728000909) 全绿，已发布[八项正式资产](https://github.com/lich13/nexushub/releases/tag/v1.3.7)。已使用正式 Linux 服务包升级远端 API，服务版本为 1.3.7、systemd 为 active、回环 healthz 成功；已安装正式 macOS ARM64 App，bundle 版本为 1.3.7 且启动成功，并保留 1.3.6 回退 App。安装和部署临时文件已清理，未自动启用 Claude hooks。

## 待用户验收

在本机和远端“更新与维护 → DSH 归档清理”预览归档，退出对应机器的 DSH 后清理专门测试线程，重开确认归档消失且其他线程保留。使用 Claude 时确认恢复执行后不再收到旧失败提醒；真实线程删除及手机实收由用户验收。

在三个 Provider 中测试 ⌘／Shift 多选、复选框及菜单；用专门测试线程检查空线程、待回复、回退 rollout 和整树删除，确认 Claude 运行时仍能管理同目录的其他历史会话。

随后在本机 Probe 设置中确认安装 Claude hooks，验证完成、问题、授权和失败提醒，完成后应无空闲重复提醒；Codex 同回合多个独立问题仍应分别通知。手机实收与日常交互未由助手代验。

1.3.7 真实验收：关闭占用目标的 codex 写入者后，预览并删除线程 `01a16116`，确认主 rollout、writer lock、shell snapshot 和 visualization 旁路均消失；验证原冲突 Claude 会话仍可改名和删除。浏览器回归与 macOS 有头性能检查以 GitHub Actions 为准，确认验收过程不需要或启动系统 Google Chrome。
