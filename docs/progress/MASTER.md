# NexusHub 当前状态（维护者）

## 当前版本

当前源码版本为 1.3.3 候选，正式维护版本仍为 1.3.2；基线为 `f74d32a / v1.3.2`。候选提交 `1815f3f` 已按原 bundle 身份覆盖安装，通过代码签名、527 项产物隐私检查和启动检查；真实功能仍待用户验收。该提交只在已通过三项 CI 的 `0874a48` 上同步本阶段维护记录，产品源码一致；CI 的 Chromium/WebKit 线程切换 p95 为 49.9/85ms，均低于原有 100ms 门槛。正式 1.3.3 未发布。正式发布状态与八项资产以 [GitHub Release](https://github.com/lich13/nexushub/releases) 为准，完整回归以对应提交的 [CI](https://github.com/lich13/nexushub/actions/workflows/ci.yml) 为准。历史由 Git 和 Release 保留。

## 当前改动与证据

只读实测确认本机桌面 build 20586、bundled CLI 0.163.0-alpha.10.2。临时 helper 带线程 ID 的 `app/list` 返回 `-32600 / thread not found`；不带线程 ID 的冷读约 29–32 秒，同一 helper 第二页约 108ms；`model/list` 包含 `max`。未发送真实回合、采集用户正文或修改原生会话，这些证据不等于两端功能验收。

| 项目 | 核实与实现 |
|---|---|
| 安全诊断 | 增加本机内存诊断与复制入口，连接失败也可读取。仅保留固定分类、安全方法/版本/code；自由文本占位。桌面 build、CLI 版本读取失败为未知，不落盘或上传。候选反馈后的复制改由 Rust 写入安全 DTO，界面显示成功/失败；真实剪贴板操作待验收。 |
| R1 应用及目录 | 已复现并修复线程 ID 与冷读期限问题。应用请求固定 `threadId:null`，一次目录分页复用 helper，串行等待与读取分别限时。五种目录独立缓存、错误和重试；重试仅刷新对应首页，失败不丢弃其他结果。上下文变化取消自有 helper。 |
| R2 设置 | 固定 effort 白名单确实拒绝 `max`。现由原生模型/模式目录验证，支持目录实际提供的值；core 只验证字符串边界。设置使用原生 view，失败回滚显示，切模式保留合法 effort，否则继承并提示。 |
| R3 请求结果 | 原生任意 handler 异常不能证明未执行。只将已证实的执行前拒绝记为未发送；start/steer 异常、断线和超时保持结果不明。设置、Stop、审批和问答明确拒绝不断流、不重试。 |
| R4 提交等待 | 原有 start/steer 十秒限制已由代码确认，真实超时发生率未测。按方法等待并继续处理 patch，显示提交中；其他线程连接不被共享注册锁中的 I/O 阻塞。 |
| R5 恢复与入口 | 修订缺口重建一次订阅，断线有界退避；owner 改变只恢复阅读，显式连接确认后才能写入。恢复旋转会话身份，拒绝迟到动作、频道和附件结果。打开/新建等待 owner 后连接；普通浏览不自动连接，创建结果不明不重复创建。 |
| 引用与完整流程 | 新引用由 Rust 校验 UTF-8 区间后按已核实的原生链接编码发送，保留空 text_elements、旧队列格式及审批门控。细分附件数量、大小、格式、身份变化和容量错误；只读时仍显示暂停/unknown 队列。成功、切换和重连清除过期界面错误，unknown 仍由队列显示。 |
| R6 版本 | 未发现本机方法版本漂移，白名单不变。非 20586 build 只提示未验证；具体方法版本不匹配才停止该操作，不探测其他版本。 |

实现参数、安全边界、兼容字段和未核实项集中在[架构说明](../ARCHITECTURE.md#本机-codex-桌面协作)。API 协议仍为 2；新增诊断命令仅注册到 macOS，目录刷新、独立 modes 及视图连接信息均已同步契约。可选 `confirmOwner` 区分用户明确连接与隐藏后重新订阅，旧请求不能自动确认已有 worker 的新 owner。

核心提交：`c1ffdef`（安全诊断）、`7b3193c`（目录与原生设置）、`9651cef`（附件校验错误）、`1bdc6b8`（请求语义、恢复与界面代际隔离）。没有批量升级依赖；Tokio 测试时钟仅启用现有开发依赖特性。

## 本机必要检查

下表针对上述提交组成的同一产品/测试源码执行；检查期间仅整理增量提交，没有继续修改被检查源码。核心阶段版本为 1.3.2，收尾的版本、文档和候选构建检查另行记录。命令入口见 [README 开发](../../README.md#开发)。

| 范围 | 命令与最终结果 |
|---|---|
| Rust core | `cargo test -p nexushub-core --locked` 退出 0，741 通过、0 失败，10 个既有 ignored 未运行。`cargo clippy -p nexushub-core --all-targets --locked -- -D warnings` 退出 0。 |
| Tauri | `cargo test --manifest-path src-tauri/Cargo.toml --locked` 退出 0，119 通过，包含 entry guard。`cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings` 退出 0。 |
| 格式 | `cargo fmt --all -- --check` 和 `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` 均退出 0。 |
| WebUI | `corepack pnpm@12.11.2 --dir webui typecheck`、`test`、`build:tauri` 均退出 0；最终 55 个文件、735 项测试通过，生产构建转换 1,962 个模块。 |
| 契约 | `node scripts/check-wire-types.mjs` 退出 0，核对 16 个文件；`node --test scripts/test-wire-types.mjs` 退出 0，8 项通过。Rust wire contract 6 项通过，亦包含在最终完整 core 命令中。 |
| 配置 | `python3 scripts/check-versions.py`、`python3 scripts/check-workspaces.py` 均退出 0；此阶段核对的是 1.3.2 manifest/lock，MSRV 和八项共享依赖解析一致。 |
| 安装与工程脚本 | `bash scripts/test-install-script.sh` 退出 0，包含 ShellCheck、13 项安装安全、9 项签名、5 项安全审计策略、19 项隐私脚本测试及版本/workspace 检查。此阶段输出的安装边界版本为 1.3.2；不代表已生成或安装 1.3.3。 |
| 隐私与差异 | `python3 scripts/privacy-check.py --git-objects` 和 `git diff --check` 退出 0；覆盖核心提交、当前文档工作树和可达 Git 对象，无失败项。 |

新增和既有针对性覆盖包括：50 条诊断与脱敏、仅本机入口；无线程应用请求、32 秒模拟冷读、分页同进程、刷新隔离、目录取消；动态 effort、模式局部失败、设置回滚；固定错误分类、业务拒绝不断流、90/30/10 秒期限及 15 秒延迟成功；一次失步重建、五次退避、新 owner 只读与手动确认、精确 message ID 核对、不重发 unknown、旧代际丢弃；20 秒自动连接、创建不重复；附件授权/容量及迟到结果、长读取尾刷新、四类请求双侧撤下和成功后的错误清除。破坏性场景仅使用隔离 fixture。

本轮不是首次全绿：中间编译与测试 fixture 问题修复后已复验；完整 core 和 Tauri 构建曾因本机磁盘满退出 101，停止构建后仅清理本轮被后续构建替代的 11 个增量快照，保留最新缓存、依赖和正式产物。随后完整 core 为 740 通过、1 项契约签名快照失败、10 ignored（101）；同步 rustfmt 后的适配层签名清单并审核后，专项 wire 6 项通过，再串行运行上表完整 core、clippy 和 Tauri 测试，均退出 0。Tauri 连接边界专项一度为 4 通过、1 失败（20 秒边界多尝试一次），修正绝对期限检查后包含在最终 Tauri 完整通过结果中。WebUI 的 734 项结果已由复制兼容修复后的 735 项完整测试替代。

未执行真实回合、两端审批、OS 拖放、真实剪贴板、视觉或日常使用验收。完整回归、跨平台构建及 Chromium/WebKit、macOS headed 性能交现有三项 CI；本机结果不等于这些 CI 通过。

## 收尾检查与候选

`af30c9c` 同步 1.3.3 版本与六份文档，两份 lock 仅更新自身包版本。对应 [CI 39026711456](https://github.com/lich13/nexushub/actions/runs/39026711456) 的 Frontend checks 与 macOS Tauri checks 成功，Backend checks 在安全审计步骤退出 1：Windows 可选 `zip 4.6.1` 命中 RUSTSEC-2026-0367/0368/0369，未进入正式发布或覆盖安装。已核实 macOS updater 的 tar.gz 读取不依赖该特性；关闭未发布的 Windows ZIP 特性后仅移除 zip 及两个专属依赖，不升级其他库、不豁免审计。

首次候选生产构建退出 0，524 个文件产物隐私检查退出 0。构建中复现原有 App 缺少完整资源签名：手动补签只用于确认根因；现已把 ad-hoc 身份和既有自定义身份覆盖移到 Tauri 打包前，并增加完整产物签名校验，避免 DMG/updater 与临时副本不一致。首次候选未安装，最终安装的是下述修正后产物，不把首次失败记为全绿。

收尾修复提交为 `e05a428`（关闭无用 ZIP 特性）和 `be27bb3`（归档前完整代码签名）。对应产品/测试源码复验：Tauri fmt、119 项测试、all-targets clippy `--locked -D warnings`，2 项打包配置测试、安装脚本、版本/workspace 检查和 diff 检查均退出 0。桌面 lock 的 `security-audit.mjs cargo` 与 `pnpm` 审计均退出 0，分别为零阻塞/9 条维护告警与零告警；根 lock 未改，根审计沿用上述首次 CI 的通过结果。维护告警仍按原策略报告，没有增加忽略项或放宽门禁。

`be27bb3` 候选 app-only 生产构建退出 0，代码签名由当前配置直接生成，deep/strict 验证与 524 项产物隐私检查均退出 0。候选保留 updater 公钥和地址，但不生成正式 updater 归档；首次临时参数遗漏该覆盖曾退出 1，修正后增量构建通过。关闭旧 App 的临时等待脚本也曾退出 1，现场复核确认进程已退出且尚未交换目录；已复现其中将 `ps` 无进程返回值误判为失败的问题。两次中间退出均与成功复验分别记录。

已从确认未交换的现场续接：一次原子交换、安装后签名检查、启动命令和五秒进程存活检查均退出 0，原 bundle 身份保持。候选主程序 SHA-256 为 `c110cc061609f4aad9ad6220e2a6417cae252971e3b987d927cdfcc30a6e8ed4`，bundled helper 为 1.3.3；旧 1.3.2 App 仅留一套回退材料，未回滚业务数据。安装暂存、失败生成的 unsigned updater 归档及临时构建文件已精确清理，源码占位资源已恢复。候选后的当前文档同步不改变该产品源码；完整三项 CI 随对应提交执行，以 Actions 结果和对话交接为准，不能用此前两项成功代替完整门禁。

## 候选与十项待用户验收

当前候选 `1815f3f` 已按原 bundle 身份覆盖安装。上一候选 `479acbd` 的反馈保留为历史证据；以下十项均需在当前候选重新验收，缺少对应应用、审批或表单时记为未测，不能记通过。

前提：本机 Codex 桌面打开同一主线程，NexusHub 选择本机；远程、归档和子线程仍不可写。

| 项目 | 操作与预期 | 自动证据 | 实机结果 |
|---|---|---|---|
| 1 连接与诊断 | 在 Codex 中打开后等待连接；复制诊断应只有安全类别及版本，不含正文或路径。 | 连接期限、诊断入口/脱敏、独立 pasteboard 与复制反馈/迟到结果测试 | 当前候选已安装；未读取用户剪贴板，实际写入结果待用户验收 |
| 2 模型与模式 | 选择原生目录提供的模型、max/其他 effort、Plan/默认；两侧值一致，失败恢复真实选择。 | 目录成员、effort 边界、设置回滚与部分失败测试 | 待用户验收 |
| 3 目录 | 分别读取、搜索和重试技能/插件/应用；空目录有提示，单项失败不阻断发送或清空其他结果。 | helper 合成进程、32 秒时钟、分页/缓存/刷新/取消测试 | 用户反馈插件/应用慢或失败；只读 helper 重测插件约 2.5 秒、技能约 0.06 秒、应用首页约 32.8 秒，不能据此认定 UI 通过 |
| 4 内联引用 | 输入 $ / @ 并选择技能、插件、应用；发送后桌面显示引用，实际调用对应能力。 | 原有 input fixture、UTF-8 区间、handle-only payload 及新增应用选择测试 | 上一候选反馈显示和实际调用失败；当前候选源码已核实链接编码与附件 context，实际显示/调用仍待用户验收 |
| 5 附件 | 选择文件、粘贴图片、拖入文件；有效内容可发送，超限/变化得到具体安全提示。 | 原有原生剪贴板 fixture、身份/符号链接与新增容量/错误测试 | 上一候选截图曾出现无主动附件操作的错误提示；当前候选已安装，后台绑定/位置更新与前台授权检查的实机行为待用户验收 |
| 6 发送与控制 | 发送、运行中补充、排队、停止；桌面执行可见，Stop 先暂停队列，unknown 不重发。 | session/worker、队列和提交状态测试 | 待用户验收 |
| 7 审批同步 | 命令/文件/问答/MCP 表单分别从任一侧完成；另一侧约一秒内撤下，不扩展决策。 | 合成 socket 双侧完成与生命周期撤下测试；真实延迟未测 | 待用户验收 |
| 8 重启恢复 | 重启桌面后恢复只读；新 owner 点击连接确认才可写，旧队列暂停、unknown 保留。 | 退避、失步、确认与代际测试 | 待用户验收 |
| 9 新线程 | 创建空线程并打开，自动连接后发送第一条消息；后续打开失败不重复创建。 | create 提交不确定结果、意图有效期、StrictMode 与新建组件测试 | 待用户验收 |
| 10 错误与长回合 | 检查设置/目录/附件错误、长回合尾部刷新和成功后的错误清除；诊断包含安全方法与 code；已有原生 Goal 的目标和状态同步，管理入口打开 Codex。 | 错误映射、deadline、尾刷新、状态清理及只读 Goal 投影/展示测试 | 上一候选附件错误误报未通过；当前候选与 Goal 状态待用户验收 |

普通文件的完整原生 wire、其他桌面 build、未核验审批决策、仅 IPC pending 的通知缺口及真实视觉/滚动仍为【未核实】；没有证据的编码和通知改动保持跳过。完整 helper 提交后超时分支及选择文件瞬间发生变化的细分文案尚未单独故障注入；已有保守未知结果与发送前身份变化测试，不将其写成上述实机通过。

## 本次反馈核查与追加范围

2026-10-17 核查仍为桌面 build 20586。附件接收区的自动 bind/bounds 与主动 paste 共用窗口聚焦检查；WebView 失去窗口焦点时未必进入 document.hidden，自动更新的错误会显示到输入区。`@` 目录依次等待应用、插件，慢应用会延迟插件展示。原生 owner 的 start 处理器从 turnStart.context 读取 attachments，旧发送构造放在 request；现已修正这处元数据差异，不能直接推断为所有引用失败的唯一原因。诊断按钮成功时仅清空错误，没有成功提示；真实写入失败原因尚未复现。

按用户给定的 [codex-mobile-bridge 源码](https://github.com/try2love/codex-mobile-bridge/tree/70a275b0eabea6f3bda333f35ef925741927bd11) 对照目录缓存和 follower 发送。该项目的 Goal 写入另用 app-server 元数据通道，并非 follower Goal 方法；本机已核验的方法表也没有 Goal 写入。用户已确认本轮只同步原生 Goal 的目标与状态，并沿用“在 Codex 中打开”入口进行原生操作，不恢复已退役的 NexusHub Goal 调度器。UI 按用户提供的原生截图整理添加菜单、引用列表、模型/思考强度、发送按钮及局部反馈；输入区与只读 Goal 投影现已实现；合成验证和候选实机验证分别记录，不宣称已解决所有真实能力调用问题。

本次核心改动：

- 附件接收在窗口失焦时解绑，旧 bind/bounds 结果不污染当前输入框；相同 handle 不重复发更新，主动粘贴的安全检查保留。
- 复制诊断使用现有本机命令的可选 copy 字段，由 Rust 写入安全 JSON 并显示成功/失败；唯一测试 pasteboard 不触碰用户剪贴板。
- 插件先发起读取，应用/插件结果与重试独立；Rust helper 仍串行，原生应用冷读约 30 秒的上游耗时未声称消除。
- 已核实原生 composer dG/uG/getPersistedText 将引用序列化为链接；新提交由 Rust 按授权 handle 构造链接并保留结构化引用。旧加密队列没有新标记时保持旧文本/区间，不重写密文。Start 附件放回原生 context。
- 只读 Goal 随原生 snapshot/patch 更新；状态/目标有界展示，通过原生打开入口管理，没有 Goal 写入命令。添加菜单、引用标签、模型摘要与圆形发送按钮已整理，视觉仍待用户验收。

增量提交：`4cd9049` 附件失焦、`d0e6e97` 独立目录、`9cfa71d` 原生 Goal 投影、`106c49c` 原生诊断复制、`8339985` 引用与旧队列、`0064785` 输入区展示。以下结果对应这些提交组成的产品与测试源码；此前候选的完整测试和 CI 不能作为本次修复的完整回归证明。

| 范围 | 本次必要检查与结果 |
|---|---|
| Rust core | `cargo test -p nexushub-core --locked codex::desktop`：123 项通过；`db::desktop_submissions`：15 项通过；`--test wire_contract`、`--test contract_registry`：各 6 项通过。随后仅调整 Goal 状态表达式以满足 Clippy，`goal_` 复验 5 项通过，保留未知状态拒绝推断。全部退出 0。 |
| Tauri 与静态检查 | `cargo test --manifest-path src-tauri/Cargo.toml --locked codex_desktop::`：41 项通过；两套 workspace 的 fmt check，以及 core/Tauri 的 all-targets Clippy `--locked -- -D warnings` 均退出 0。 |
| WebUI | `corepack pnpm@12.11.2 --dir webui typecheck` 退出 0；六个定向 Vitest 文件共 71 项通过，覆盖引用交互、设置、Goal、目录、附件接收与桌面 API。包含 thread/session 切换后的迟到复制结果、原生选择菜单的临时失焦、插件先展示与独立重试。 |
| 共享契约与配置 | `node scripts/check-wire-types.mjs` 核对 16 个文件、`node --test scripts/test-wire-types.mjs` 8 项通过；版本/workspace 检查、7 项 workspace 策略测试及 2 项 Tauri 打包配置测试均退出 0。 |
| 安装与隐私 | `bash scripts/test-install-script.sh` 退出 0，61 项通过；`python3 scripts/privacy-check.py --git-objects` 退出 0，核对当前树、暂存文档与可达历史共 3,092 项，无失败；工作区与暂存差异检查均退出 0。未执行真实安装或部署。 |

本次并非所有命令首次通过：引用 fixture 首先误把空格放进只接受非空白 URI 的 app 样本，之后两处转义期望写错；改用临时 skill 文件覆盖空格并按源码修正后通过，未放宽授权校验。一个测试断言排版曾使 fmt 退出 1；Goal 状态表达式使 Clippy 退出 101，保留六状态白名单修正后复验通过。独立 pasteboard 测试最初缺少显式 unsafe 编译失败也已修正，并包含在上述 Tauri 通过结果中。前端最终六文件检查没有首次失败。

上述核心阶段未重新执行全量 Rust/WebUI、浏览器、headed 性能或 App 生产构建，也未发送真实回合。后续收尾的构建、安装与 CI 结果见下节；原生调用、复制、Goal 状态、视觉和十项流程均仍待用户验收。

用户追加取消旧 Rust 工具链兼容检查：`c94eb5d` 将两套 workspace 最低声明与固定开发工具链统一到 1.99，CI 不再安装另一套旧版本；依赖与锁文件未变化。workspace 检查 7 项测试、实际 workspace/version 检查、两套 cargo metadata --locked --no-deps 及 diff 检查均退出 0，受影响 Rust/Tauri 编译检查见上表。历史 1.2.23 发布说明保留当时的最低版本事实，不是当前构建依赖。

## 修订候选与待处理门禁

2026-10-18 收尾对应产品提交 `479acbd311a03b1874a3a813ae8129f494d957b0`，已推送至 main。复用既有依赖、release 缓存和生产 WebUI 构建；本机 release 保留原有 `--remap-path-prefix`，将编译用户目录映射到 `/build`，避免注册表和项目路径进入产物。helper release 构建及 Tauri app-only 构建均使用 `--locked`，退出 0；后者保留 updater 公钥和端点、不生成正式 updater 归档。版本 1.3.3、bundle 身份及 arm64 检查、deep/strict 代码签名、527 项产物隐私检查均退出 0。

首次本机构建曾遗漏上述路径映射，产物隐私检查退出 1，未安装该产物；恢复既有参数后增量复验通过。helper 包装命令也曾因临时路径变量展开错误退出 22，Cargo 自身退出 0，随后复用其结果续接；这些中间失败不记为首次全绿。

修订候选经正常退出旧 App、同卷暂存、一次原子交换后安装。安装后版本、签名、helper、启动命令和五秒进程存活检查均退出 0；主程序 SHA-256 为 `ef23d37f264266698d88ffac4b433ecb1f215e43a5657ec8aecaee67a2de4978`，helper 为 `5e9266b086b2890e7a0d780398d231bccfe94d63d136d5649a04401bf440d9a5`。保留最新候选和一套正式 1.3.2 App 回退材料，没有回滚业务数据或升级远程 API。失败候选、换下的旧候选、预览及本轮暂存已精确清理，清理前磁盘占用合计 77,684KiB；文件系统可用空间增量含同期系统活动，不等同于本轮回收量。

| 检查 | 实际结果 |
|---|---|
| 本机 headed 性能 | 补齐锁定版本对应的 Playwright 浏览器后，复用生产构建运行 README 的串行、关闭 trace 命令；Chromium/WebKit 两项均退出 0。展开 p95 为 31/32ms、滚动 31.7/31ms、缓存线程切换 30.8/50ms，均低于 100ms。此前一次执行因两种浏览器缓存缺失退出 1，未取得测量。 |
| [CI 39056782588](https://github.com/lich13/nexushub/actions/runs/39056782588) 第一次 | 同提交 Frontend/Backend 成功；macOS 性能退出 1，线程切换 Chromium/WebKit p95 为 125.5/115ms。展开与滚动通过。 |
| 同提交 CI 第二次 | 本机复现通过后仅复核失败作业一次，Frontend/Backend 保持成功；Chromium 切换 66.7ms 通过，WebKit 切换 114ms 仍失败（最大 116ms）。其展开 52ms、滚动 36ms 通过；后续 Tauri test/clippy 均 skipped，不能记为通过。未继续重跑或改变门槛。 |

此次状态文档更新的 `git diff --check` 退出 0。文档更新后的 Git 对象隐私扫描已执行，但结果收集丢失会话句柄，退出码与条目数无法取回；不把进程结束当作通过，也未因此重跑。此前产品源码、CI 前后端与候选产物的隐私检查结论保留，后续源码变更仍须执行相应检查。

本机结果不能替代 CI，因此返回实现阶段定位 WebKit 在 CI 的持续超标，不将其推定为偶发波动或新输入区的确定缺陷。后续修复证据见下节。候选十项仍待用户验收，正式发布继续受同提交三项 CI 与约定真机验收条件约束。

## 线程切换性能修复

2026-10-18 返回实现时，`672ef97` 与已安装候选具有相同产品源码。本机 WebKit 原样本再次得到切换 p95 50ms（20 次、72 条消息）。临时 DOM 诊断确认每次切换通常调用三次 `onRendered`，约 432 次节点位置读取来自 `snapshot → capture → onRendered`；即使跟随底部，也在扫描全部时间线节点。WebKit 未提供 longtask/long-animation-frame 数据，不能把整个 CI 耗时归因于此路径；单独绕过布局可见性读取也没有获得收益。临时诊断和输出已清理，正式测试与 100ms 门槛未修改。

`historyScrollCoordinator.capture` 在跟随底部时只更新滚动位置，不查找未使用的阅读锚点；向上阅读、搜索和分页仍先关闭跟随再捕获锚点。新增两项回归覆盖底部追加、上滚翻页与轮询的可见偏移。在 `webui` 中定向执行 `pnpm exec vitest run src/lib/useHistoryScroll.test.ts` 共 11 项，退出 0；此前测试脚本的参数分隔符没有生效，意外执行的 56 文件/758 项也退出 0，之后未再执行全套。

`corepack pnpm@12.11.2 --dir webui build:tauri` 包含类型检查，退出 0。随后复用生产构建执行 README 的串行 headed、trace-off 性能命令，两项均退出 0：Chromium/WebKit 展开 p95 为 30.8/33ms、滚动 30.5/30ms、线程切换 31.8/45ms，分别使用原有 60/20/20 次样本。该修复尚未打入已安装候选；本机测量不能替代新提交 CI 或真实滚动验收。

既有 `activity-timeline.pw.ts` 中的 `Codex earlier-page loading preserves all tool rows and the open group` 和 `native pagination inserts older activity while preserving the visible anchor` 两个模式分别在 Chromium/WebKit 运行，4 项通过、退出 0；覆盖工具组分页、折叠选择与可见锚点。只使用合成 fixture，预览和本轮测试输出已清理。

产品提交 `0874a48440d9f2663618f55bfb5b21f0586aca09` 已推送，[CI 39060232192](https://github.com/lich13/nexushub/actions/runs/39060232192) 的 Frontend、Backend、macOS Tauri 三项全部成功，watch 退出 0。锁定依赖的 workspace 测试汇总为 960 通过、10 忽略、0 失败，Tauri 测试 121 通过、0 失败；两套 all-targets Clippy 均实际执行并通过。CI Chromium/WebKit 展开 p95 为 43.1/36ms、滚动 39.3/32ms、切换 49.9/85ms；WebKit 切换最大值为 91ms。与上一提交复核的 114ms 相比，WebKit p95 下降 29ms；不同 CI 运行仍存在环境波动，不能把全部差值视为单项代码改动的严格因果收益。门槛、样本和浏览器配置保持不变。

提交前 `python3 scripts/privacy-check.py --git-objects` 检查 3,099 项、无发现、退出 0，`git diff --check` 退出 0。本节仅同步实际证据，不改变已验证产品源码；候选更新和用户十项验收按原定阶段继续。

## 保留边界与交付条件

桌面仍是唯一执行 owner；不回退到独立 CLI、PTY、云 API 或 remoteControl，不直接改写原生会话和队列。API 协议 2、旧队列字节兼容、Keychain、认证比较、禁止重定向、远程 allowlist、tombstone 和 systemd 权限边界保持。九个桌面专属命令均不向 Linux RPC 开放。

候选十项验收通过、对应提交三项 CI 成功后才发布正式 1.3.3；未测项需要明确处理，不能当作通过。八项资产、校验和、签名及平台映射核验后，先升级远程 API，再安装正式 macOS App；仅做部署、版本和启动检查。正式产物、会话、连接、凭据、私有附件与共享缓存保留，只清理本轮明确归属的暂存。
