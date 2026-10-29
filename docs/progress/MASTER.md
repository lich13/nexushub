# NexusHub 当前状态（维护者）

## 当前版本

当前维护源码版本为 1.5.0，基线为 `2c3cb22 / v1.4.0`。本版恢复原生二进制公网 IP 隐私门禁，收紧 Antigravity 与 DeepSeek Harness（DSH）子会话删除的执行端闭包重算、运行态／锁占用拦截与首错即停残留报告；统一五类 Agent「复制路径」最优交接文件（补齐 Grok Build 行菜单与顶栏复制路径）；前后端移除顶部五类状态筛选栏并改为底部按需加载的轻量折叠「已归档」抽屉；将「多选线程」按钮与行首复选框切换为访达风格 `⌘`／`Shift` 多选与多选右键／快捷键批处理；统一五类 Agent 连续中间过程的单框折叠，修复左侧用户时间线（横线）刷新丢失与折叠跳转，并消除 Antigravity 在后台任务、定时器或子代理尚未收敛时提前发送完成 Bark 提醒的问题；同时深度重写前端视觉细节。API 协议保持 2；macOS App 仍是唯一界面。

历史实现与交付由 Git 和 [Release](https://github.com/lich13/nexushub/releases) 保留。完整回归、跨平台构建和既有性能检查以发布提交对应的 [CI](https://github.com/lich13/nexushub/actions/workflows/ci.yml) 为准。

## 已实现范围

| 工作块 | 当前行为 | 核心提交 |
|---|---|---|
| 隐私门禁与子会话删除加固 | 消除 x86_64 端口解析跨字面量立即数片段并恢复原生二进制公网 IP 隐私扫描；Antigravity 与 DSH 删除执行端重新核验真实子代理闭包，逐项检查根与子会话运行／锁状态，在线 RPC 或文件系统首错即停并返回 residuals。 | `13d7de2` |
| 五类 Agent 最优交接路径与筛选收敛 | Antigravity 优先复制完整纯文本转录日志，Grok 新增 `handoff_path`（优先 `chat_history.jsonl`）与复制路径菜单，Codex 覆盖活跃／归档 rollout，Claude 与 DSH 保持明文优先；移除顶部状态筛选栏，提供底部轻量折叠「已归档」抽屉并支持打开归档详情。 | `9847927`, `6170acf` |
| 访达多选、过程折叠、时间线与通知收敛 | 移除多选按钮与复选框，统一访达风格 `⌘`／`Shift` 多选、右键批处理菜单与 `⌘A`／`Escape`／`⌘⌫`；同回合交替思考与工具折叠为单条摘要（单执行组零 DOM 膨胀）；分页轮询保留首页 `timeline` 并补全在线批注与首轮提示；Antigravity 完成 Bark 等待根与子轨迹、后台任务及子代理全部收敛。 | `9847927`, `6170acf` |
| Antigravity／DSH 会话工作区 | 七项导航包含 Antigravity 与 DeepSeek Harness；支持分页阅读、时间线、搜索、双击改名、归档／恢复、复制 ID、复制路径、永久删除（无复制恢复命令）及多选批处理；DSH 写操作遵循 `session.lock` 与宿主进程离线安全守卫。 | `04dcb98`, `6170acf` |
| ChatGPT macOS App 风格重构 | 统一浅／深色中性侧栏与画布、访达多选圆角高亮、右侧紧凑胶囊用户气泡、无框助手 Markdown 与圆角代码块、低干扰过程折叠行、重绘左侧横线时间线、圆角菜单／弹窗及卡片化 Probe／设置面板，保持 WCAG `>= 4.5:1` 对比度、`48px` 折叠导航与 WebKit 帧预算不变量。 | `04dcb98`, `6170acf` |
| 本机 Antigravity／DSH 通知 | 原生执行终态、WAITING 交互、v4 明文／多帧压缩读取；四类独立开关、启用基线、身份去重、回复和审批解除、旧失败失效。只在本机运行。 | `v1.3.11` |
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
| 线程消息排序 | Codex、Claude Code 和 Grok Build 均按用户或助手最后一条对话消息时间倒序；工具、改名和其他元数据更新时间不改变顺序，未知时间保持稳定。 | `5770133` |

实现、安全边界、旧字段与旧队列兼容见[架构说明](../ARCHITECTURE.md)。删除按确认目标逐项执行，失败返回 residuals 并继续其他目标；归档、恢复和改名仍沿用各自规则。Claude 删除不触碰工作目录或共享文件中的非目标记录。

## 必要验证

1.5.0 本机与主 CI 已通过 `nexushub-core`／`nexushub-webd`／`src-tauri` 全量测试及两端 Clippy（含子会话越界注入、运行／锁拦截、在线 RPC 失败零误删与部分文件失败残留报告、五大 Agent 交接路径优先级、Antigravity 后台任务／定时器／子代理未收敛抑制 Bark 完成提醒）、Rust／TypeScript 线协议契约锁、WebUI 65 文件 833 项 Vitest 与类型检查、安装脚本及恢复原生二进制 `ipv4` 公网 IP 检查后的发布载荷隐私扫描；GitHub Actions 三项主 CI 已通过七工作区浅／深色 × 八视口零溢出与对比度 E2E、底部「已归档」抽屉与访达 `⌘`／`Shift` 多选批处理 E2E、五类 Agent 上下文菜单（含 Grok 复制路径）E2E 以及 macOS 有头 WebKit 交互帧预算检查。未使用真实用户会话执行写操作或删除测试。

1.4.0 本机与主 CI 已通过 `nexushub-core`／`nexushub-webd`／`src-tauri` 全量测试及两端 Clippy、Rust／TypeScript 线协议契约锁、WebUI 64 文件 825 项 Vitest 与类型检查、安装脚本与发布载荷隐私扫描；GitHub Actions 三项主 CI 已通过七工作区浅／深色 × 八视口（含 `320px`）零溢出与对比度 E2E、五项上下文菜单（确认不含“复制恢复命令”）E2E 以及 macOS 有头 WebKit 交互帧预算检查。未使用真实用户会话执行写操作或删除测试。

1.3.11 本机已通过 core 全量测试、WebUI 全量测试及类型检查、Tauri 测试和 Clippy；新增实际 CLI 回环用例验证 DSH 四类投递、重启去重及处理交互／恢复后的旧提醒停止重试。Antigravity 原生 API 仅作只读结构核对；事件识别、请求身份、服务不可用及非等待状态由合成用例验证。Zstandard 依赖升级至修复版 0.14.3／zstd-safe 8.2.0，保留现有依赖安全门禁。正式构建与完整回归以对应提交 CI 为准，真实交互及手机实收待用户验收。

以下证据覆盖 1.3.10 核心实现及收尾；最终提交 [`6811cb4`](https://github.com/lich13/nexushub/commit/6811cb4e7a8d7ea763191640b2badd42e6e93807) 的[三项主 CI](https://github.com/lich13/nexushub/actions/runs/40045161446) 与 [`v1.3.10` Release CI](https://github.com/lich13/nexushub/actions/runs/40046131999) 已通过。

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

1.5.0 核心提交 [`13d7de2`](https://github.com/lich13/nexushub/commit/13d7de25ff8cbc1625736b306c7cfad5d88f7431)、[`9847927`](https://github.com/lich13/nexushub/commit/9847927ffc20406ba71718dc9871c6ee6a346880)、[`6170acf`](https://github.com/lich13/nexushub/commit/6170acf393622c9bbb9f1d002ce147d2820ac166) 与发版提交 [`016db18`](https://github.com/lich13/nexushub/commit/016db18c08b9bc91d68b661d6248b538b500c3cb) 的[三项主 CI](https://github.com/lich13/nexushub/actions/runs/40815957745)（macOS WebKit 性能作业重跑后）与 [`v1.5.0` Release CI](https://github.com/lich13/nexushub/actions/runs/40817452620) 全绿，已发布[八项正式资产](https://github.com/lich13/nexushub/releases/tag/v1.5.0)（Linux 与 macOS 原生二进制均通过恢复后的公网 IP 隐私门禁）。已按先云端后本机顺序，先使用正式 Linux 服务包升级远端节点 `nexushub-webd 1.5.0`，service 为 `active`、health 为 `{"ok":true}`；再使用正式 `NexusHub-1.5.0-darwin-arm64.dmg`（SHA-256 与签名校验通过）原地覆盖安装本机 macOS App，bundle 版本及本机监控进程均为 1.5.0 且启动运行，没有保留旧副本。真实用户会话未执行删除，待用户验收。

1.4.0 发布提交 [`04dcb98`](https://github.com/lich13/nexushub/commit/04dcb98b1e7d452dda5ffe28ba6f9dfd5aba065e) 的[三项主 CI](https://github.com/lich13/nexushub/actions/runs/40648048212)与 [`v1.4.0` Release CI](https://github.com/lich13/nexushub/actions/runs/40648932371) 全绿，已发布[八项正式资产](https://github.com/lich13/nexushub/releases/tag/v1.4.0)。已按先云端后本机顺序，先使用正式 Linux 服务包升级远端节点 `nexushub-webd 1.4.0`，service 为 `active`、health 为 `{"ok":true}`；再使用正式 `NexusHub-1.4.0-darwin-arm64.dmg`（SHA-256 与签名校验通过）原地覆盖安装本机 macOS App，bundle 版本为 1.4.0 且启动运行，没有保留旧副本。真实用户会话未执行删除，待用户验收。

1.3.11 发布提交 `1af6332` 的[三项主 CI](https://github.com/lich13/nexushub/actions/runs/40361843004)与 [Release CI](https://github.com/lich13/nexushub/actions/runs/40363304134)已通过，发布[八项正式资产](https://github.com/lich13/nexushub/releases/tag/v1.3.11)。已使用对应 Linux 产物部署远程服务器，服务版本为 1.3.11、service 为 active、health 为 `{"ok":true}`；已使用对应 DMG 原地安装 Mac，版本为 1.3.11 且启动运行，没有保留旧副本。本机 Antigravity／DSH 总开关及四类提醒已按授权开启，复用原有 Bark 配置；首次扫描无读取错误并跳过历史，监控进程运行。云端两项开关关闭。真实交互及手机实收仍待用户验收。

1.3.10 已发布[八项正式资产](https://github.com/lich13/nexushub/releases/tag/v1.3.10)。已使用正式 Linux 服务包部署远程服务器 `nexushub-webd 1.3.10`，service 为 `active`、health 为 `{"ok":true}`；已使用 `NexusHub-1.3.10-darwin-arm64.dmg` 原地安装 macOS App，bundle 版本为 1.3.10，签名核验通过且启动运行，没有保留旧副本。

1.3.9 的[三项主 CI](https://github.com/lich13/nexushub/actions/runs/39865303352)与 [`v1.3.9` Release CI](https://github.com/lich13/nexushub/actions/runs/39866444443) 全绿，已发布[八项正式资产](https://github.com/lich13/nexushub/releases/tag/v1.3.9)。已使用正式 Linux 服务包部署远端 API，版本为 1.3.9、service 为 active、health 为 `{"ok":true}`；已用 `NexusHub-1.3.9-darwin-arm64.dmg` 原地安装 macOS App，校验文件和签名通过，bundle 版本为 1.3.9 且启动成功，没有保留旧副本。安装与部署脚本已清理各自临时文件；真实用户会话未执行删除，仍待用户验收。

1.3.8 已同步现有 manifest 和两份 lock 中的自身版本，发布说明使用中文六节，保留八项正式资产：macOS DMG／校验、updater／签名、latest.json，以及 Linux API 服务包／校验／签名。核心修复提交 [`c645e44`](https://github.com/lich13/nexushub/commit/c645e44885a43f34ebddefb3e3f172c84a38a88b) 的[三项主 CI](https://github.com/lich13/nexushub/actions/runs/39840632543)（macOS WebKit 性能作业重跑后）全绿；[`v1.3.8` Release CI](https://github.com/lich13/nexushub/actions/runs/39843284210) 全绿并发布[八项正式资产](https://github.com/lich13/nexushub/releases/tag/v1.3.8)。已使用正式 Linux 服务包部署远端 API，版本为 1.3.8、service 为 active、health 为 `{"ok":true}`；已使用正式 macOS ARM64 DMG 原地安装 App，bundle 版本为 1.3.8 且启动成功，没有保留旧副本。收尾未创建预览服务或 SSH 转发；两个真实样例未执行删除，保留待用户验收。

上一版交付记录：提交 [`51dfcd5`](https://github.com/lich13/nexushub/commit/51dfcd5225eef215b4dade342d4330d3ad42159f) 的 [三项主 CI](https://github.com/lich13/nexushub/actions/runs/39585391199) 全绿，`v1.3.5` 的 [Release CI](https://github.com/lich13/nexushub/actions/runs/39586479754) 全绿并发布 [八项正式资产](https://github.com/lich13/nexushub/releases/tag/v1.3.5)。已使用正式 Linux 资产升级远端 API，服务版本、健康、公开 healthz、未授权 API 和退役路径检查通过；已安装正式 macOS ARM64 App，版本为 1.3.5 且启动成功。部署和安装保留既有回退材料，未自动启用 Claude hooks。

1.3.6 核心实现由 [`01c5695`](https://github.com/lich13/nexushub/commit/01c5695)、[`2aaa929`](https://github.com/lich13/nexushub/commit/2aaa929) 和 [`1cfa0a7`](https://github.com/lich13/nexushub/commit/1cfa0a7) 完成；版本、文档和 CI 启动器收尾提交为 [`7254912`](https://github.com/lich13/nexushub/commit/7254912)。该提交对应的[三项主 CI](https://github.com/lich13/nexushub/actions/runs/39716648742)（macOS 性能检查重跑后）与 [`v1.3.6` Release CI](https://github.com/lich13/nexushub/actions/runs/39717900484) 全绿，已发布[八项正式资产](https://github.com/lich13/nexushub/releases/tag/v1.3.6)。已使用正式 Linux 资产升级远端 API，服务版本、健康、公开 healthz、未授权 API 和退役路径检查通过；已安装正式 macOS ARM64 App，版本为 1.3.6 且启动成功，并保留 1.3.5 回退 App。收尾清理仅移除本轮临时文件和 NexusHub 专属缓存，实际回收 22,524 KiB；未自动启用 Claude hooks。

1.3.7 核心实现由 [`6ca0716`](https://github.com/lich13/nexushub/commit/6ca0716)、[`e363069`](https://github.com/lich13/nexushub/commit/e363069)、[`d73b2d4`](https://github.com/lich13/nexushub/commit/d73b2d4)、[`0d7afea`](https://github.com/lich13/nexushub/commit/0d7afea) 和 [`70602df`](https://github.com/lich13/nexushub/commit/70602df) 完成；版本、文档和发布说明收尾提交为 [`cffb3e9`](https://github.com/lich13/nexushub/commit/cffb3e9)。该提交对应的[三项主 CI](https://github.com/lich13/nexushub/actions/runs/39727312472) 与 [`v1.3.7` Release CI](https://github.com/lich13/nexushub/actions/runs/39728000909) 全绿，已发布[八项正式资产](https://github.com/lich13/nexushub/releases/tag/v1.3.7)。已使用正式 Linux 服务包升级远端 API，服务版本为 1.3.7、systemd 为 active、回环 healthz 成功；已安装正式 macOS ARM64 App，bundle 版本为 1.3.7 且启动成功，并保留 1.3.6 回退 App。安装和部署临时文件已清理，未自动启用 Claude hooks。

## 待用户验收

在本机与远端机器切换下打开五类 Agent 工作区，核验：顶部状态筛选栏与“多选线程”按钮已移除，列表底部可按需展开「已归档」抽屉并打开或恢复归档会话；五类 Agent 的「复制路径」（含 Grok Build 行菜单与顶栏更多菜单）均复制出最优纯文本／主交接文件；按住 `⌘` 单击、`Shift` 连选、右键批量菜单及 `⌘A`／`Escape`／`⌘⌫` 快捷键符合访达多选习惯；同一回合内交替的思考与工具执行折叠为单条摘要行，左侧横线时间线在五类 Agent 下均可悬停预览并准确跳转定位；在 Antigravity 发起包含后台命令或子代理的真实长任务，确认仅在整个线程彻底收敛后才收到一次完成 Bark 提醒。

在本机与远端机器切换下打开 Antigravity 和 DeepSeek Harness 工作区，核验全部／已归档列表、体积标注、分页详情、用户时间线、思考／工具折叠、⌘F 搜索，以及右键／更多菜单的五项动作（改名、归档或恢复、复制 ID、复制路径、永久删除，确认无“复制恢复命令”）、双击／F2 改名与多选批处理；用专门测试线程核验 DSH 运行态锁与宿主退出后的归档／恢复／删除保护，以及浅／深色 ChatGPT macOS App 风格视觉表现。

在本机 Probe 查看 Antigravity 和 DSH 开关，分别触发完成、失败、提问及授权并确认手机收到对应提醒；回答问题、处理审批或恢复执行后，确认旧提醒不再重试。Antigravity 原生服务须保持运行，DSH 使用标准 v4 文件存储；自动测试不代替手机实收和真实交互。

在本机和远端“更新与维护 → DSH 归档清理”预览归档，退出对应机器的 DSH 后清理专门测试线程，重开确认归档消失且其他线程保留。使用 Claude 时确认恢复执行后不再收到旧失败提醒；真实线程删除及手机实收由用户验收。

在三个 Provider 中测试 ⌘／Shift 多选、复选框及菜单；用专门测试线程检查空线程、待回复、回退 rollout 和整树删除，确认 Claude 运行时仍能管理同目录的其他历史会话。

随后在本机 Probe 设置中确认安装 Claude hooks，验证完成、问题、授权和失败提醒，完成后应无空闲重复提醒；Codex 同回合多个独立问题仍应分别通知。手机实收与日常交互未由助手代验。

1.3.7 真实验收：关闭占用目标的 codex 写入者后，预览并删除线程 `01a16116`，确认主 rollout、writer lock、shell snapshot 和 visualization 旁路均消失；验证原冲突 Claude 会话仍可改名和删除。浏览器回归与 macOS 有头性能检查以 GitHub Actions 为准，确认验收过程不需要或启动系统 Google Chrome。
