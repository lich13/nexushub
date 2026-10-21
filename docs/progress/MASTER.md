# NexusHub 当前状态（维护者）

## 当前版本

当前维护源码版本为 1.3.5，基线为 `2b443fb / v1.3.4`。本轮覆盖线程多选与菜单、Codex 整树删除、Claude 管理及专属文件删除、Bark 摘要与本机 Claude hooks。保留既有 100MiB／300MiB 颜色门槛及性能优化，API 协议仍为 2；macOS App 是唯一界面。

历史实现与交付由 Git 和 [Release](https://github.com/lich13/nexushub/releases) 保留。完整回归、跨平台构建和既有性能检查以发布提交对应的 [CI](https://github.com/lich13/nexushub/actions/workflows/ci.yml) 为准。

## 已实现范围

| 工作块 | 当前行为 | 核心提交 |
|---|---|---|
| 多选与菜单 | 三种 Provider 共用 ⌘／Shift／普通／键盘点击、锚点和 100 项上限；轮询不新增选择，切换范围清空。移除 Codex 列表重复复制答复，Claude 菜单按独立能力启用。 | `3e318f4`、`d63d6d5` |
| Codex 删除 | 支持两类空线程、无活跃写入者的待回复线程及完整子树预览；显式 includeDescendants，原生 thread/delete helper 优先，提交后错误／超时不回退、不重试，完整复检后才返回成功。 | `d37793a`、`d63d6d5` |
| Claude 管理 | 原生登记、PID／启动标识／主机进程域与打开文件核验归属；同 cwd 的其他会话不再被连带保护。兼容 warning 可管理，主文件和经父会话关联核实的专属目录共同隔离、复检。 | `3c4ba5f` |
| Bark 与 hooks | 新事件统一单条短摘要；本机 hooks 须明确安装，远端继续轮询。Claude prompt 身份核验后每回合每类认领一次，保留有效旧完成以补发，完成后不单独提醒空闲。Codex 保留逐问题身份和提醒。 | `01be4b4` |

实现、安全边界、旧字段与旧队列兼容见[架构说明](../ARCHITECTURE.md)。压缩历史、未知存储、身份或关联无法完整核验时明确拒绝；官方删除可能部分完成，不承诺跨整树回滚。Claude 改名仍只追加原生 custom-title，不删除共享存储、工作目录或续接到的另一个主会话。

## 必要验证

以下证据覆盖核心实现至 `01be4b4`，后续只复验受修改影响的检查，不把局部复验称作整套重复回归。

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

1.3.5 同步现有 manifest 和两份 lock 中的自身版本；按新增 RustSec 公告定向更新 rustls-webpki 0.103.16、webpki-roots 1.0.10 和桌面 webpki-root-certs 1.0.10，保留原有依赖审计策略。发布说明继续使用中文六节，保留八项正式资产：macOS DMG／校验、updater／签名、latest.json，以及 Linux API 服务包／校验／签名。

发布顺序为最终提交三项 CI 全绿、创建 v1.3.5、Release 构建及发布、使用正式 CI 资产升级远端 API、安装 macOS App。部署／安装只核对版本、健康和启动状态，保留现有回退材料；App 安装不自动启用 Claude hooks。目前发布和部署结果待本轮流水线完成后记录。

## 待用户验收

在三个 Provider 中测试 ⌘／Shift 多选、复选框及菜单；用专门测试线程检查空线程、待回复、回退 rollout 和整树删除，确认 Claude 运行时仍能管理同目录的其他历史会话。

随后在本机 Probe 设置中确认安装 Claude hooks，验证完成、问题、授权和失败提醒，完成后应无空闲重复提醒；Codex 同回合多个独立问题仍应分别通知。手机实收与日常交互未由助手代验。
