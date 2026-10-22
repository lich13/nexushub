# NexusHub 当前状态（维护者）

## 当前版本

当前维护源码版本为 1.3.6，基线为 `8cfff5 / v1.3.5`。本轮覆盖 Codex 压缩 rollout 删除、Claude 归属诊断与专属文件管理、Claude 单回合过程折叠。保留既有多选、菜单、通知、hooks、分页和性能行为，API 协议仍为 2；macOS App 是唯一界面。

历史实现与交付由 Git 和 [Release](https://github.com/lich13/nexushub/releases) 保留。完整回归、跨平台构建和既有性能检查以发布提交对应的 [CI](https://github.com/lich13/nexushub/actions/workflows/ci.yml) 为准。

## 已实现范围

| 工作块 | 当前行为 | 核心提交 |
|---|---|---|
| 多选与菜单 | 三种 Provider 共用 ⌘／Shift／普通／键盘点击、锚点和 100 项上限；轮询不新增选择，切换范围清空。移除 Codex 列表重复复制答复，Claude 菜单按独立能力启用。 | `3e318f4`、`d63d6d5` |
| Codex 删除 | 支持普通、归档及压缩 rollout 的完整子树预览；zstd 流式解码、目标与全局引用分离核验、明文／压缩兄弟合并识别，含压缩目标或跳过项时禁止不安全离线回退，提交后错误／超时不回退、不重试。 | `01c5695` |
| Claude 管理 | 原生登记、PID／启动标识／主机进程域与打开文件核验归属；区分会话、daemon 和 PTY 宿主，提供本机只读归属诊断。主文件和经父会话关联核实的专属目录共同隔离、复检。 | `2aaa929` |
| Claude 过程折叠 | 同一原生回合的思考、临时说明和工具活动按原顺序收进外层折叠，最终答复单独显示；搜索自动展开，分页、刷新、机器与会话切换保持稳定身份。 | `1cfa0a7` |
| Bark 与 hooks | 新事件统一单条短摘要；本机 hooks 须明确安装，远端继续轮询。Claude prompt 身份核验后每回合每类认领一次，保留有效旧完成以补发，完成后不单独提醒空闲。Codex 保留逐问题身份和提醒。 | `01be4b4` |

实现、安全边界、旧字段与旧队列兼容见[架构说明](../ARCHITECTURE.md)。压缩历史、未知存储、身份或关联无法完整核验时明确拒绝；官方删除可能部分完成，不承诺跨整树回滚。Claude 改名仍只追加原生 custom-title，不删除共享存储、工作目录或续接到的另一个主会话。

## 必要验证

以下证据覆盖 1.3.6 核心实现至 `1cfa0a7`，后续只复验受修改影响的检查，不把局部复验称作整套重复回归。

| 范围 | 已通过证据 |
|---|---|
| Rust core | 核心单元检查及修正后的定向复验；最后 selected_codex 31 项、native_probe 7 项通过，完整集成测试 226 项通过。覆盖空线程、待回复、完整子树、引用／链接／深度保护、官方部分失败、分页残留与无法核验结果。 |
| Rust webd | 199 项单元、5 项 Claude hooks CLI 集成、11 项 Codex hooks CLI 集成通过。CLI 测试启动本次 Cargo 构建的程序，覆盖并发去重、快速新回合、跨回合补发、基线、旧成功记录、旧分段队列和五秒退出。 |
| Tauri | 132 项测试与 Clippy 通过；hooks 入口只限本机，不开放 Linux RPC 或远端配置写入。 |
| WebUI | 类型检查及相关测试通过，最后四个受影响文件 96 项复验通过；多选、菜单、能力独立、机器隔离及契约断言同步更新。 |
| 静态与脚本 | 两套 Rust 格式、core／webd 与 Tauri Clippy、Rust／TypeScript 契约、现有安装脚本检查通过；提交前隐私检查和 diff 检查通过。 |

未使用真实用户会话执行删除测试，未发送真实 Bark 验收消息；功能、视觉和端到端验收由用户完成。完整跨平台和性能门禁继续由现有三项 CI 执行，没有新增性能门禁或本机正式打包。

收尾已修正删除预览浏览器 fixture 的整树能力与 includeDescendants 断言，并覆盖旧服务能力缺失／关闭时提示升级且不发删除请求。Chromium、WebKit 各 11 项相关测试、WebUI 类型检查、版本／工作区检查及现有安装脚本通过；两份锁文件的 Rust 依赖审计均无阻断项。

## 正式交付

1.3.6 同步现有 manifest 和两份 lock 中的自身版本，发布说明继续使用中文六节，保留八项正式资产：macOS DMG／校验、updater／签名、latest.json，以及 Linux API 服务包／校验／签名。

上一版交付记录：提交 [`51dfcd5`](https://github.com/lich13/nexushub/commit/51dfcd5225eef215b4dade342d4330d3ad42159f) 的 [三项主 CI](https://github.com/lich13/nexushub/actions/runs/39585391199) 全绿，`v1.3.5` 的 [Release CI](https://github.com/lich13/nexushub/actions/runs/39586479754) 全绿并发布 [八项正式资产](https://github.com/lich13/nexushub/releases/tag/v1.3.5)。已使用正式 Linux 资产升级远端 API，服务版本、健康、公开 healthz、未授权 API 和退役路径检查通过；已安装正式 macOS ARM64 App，版本为 1.3.5 且启动成功。部署和安装保留既有回退材料，未自动启用 Claude hooks。

1.3.6 核心实现已由 [`01c5695`](https://github.com/lich13/nexushub/commit/01c5695)、[`2aaa929`](https://github.com/lich13/nexushub/commit/2aaa929) 和 [`1cfa0a7`](https://github.com/lich13/nexushub/commit/1cfa0a7) 完成并推送；版本收尾提交及三项主 CI、正式 Release、远端 API 升级和 macOS 安装待本轮收尾。

## 待用户验收

在三个 Provider 中测试 ⌘／Shift 多选、复选框及菜单；用专门测试线程检查空线程、待回复、回退 rollout 和整树删除，确认 Claude 运行时仍能管理同目录的其他历史会话。

随后在本机 Probe 设置中确认安装 Claude hooks，验证完成、问题、授权和失败提醒，完成后应无空闲重复提醒；Codex 同回合多个独立问题仍应分别通知。手机实收与日常交互未由助手代验。
