# NexusHub 架构说明（维护者文档）

本文描述当前实现及维护边界。用户操作与下载见 [README](../README.md)，视觉规则见 [DESIGN](../DESIGN.md)，部署步骤见[部署手册](cloud-deploy-runbook.md)，交付状态见[当前进度](progress/MASTER.md)。

## 运行架构与模块

`desktop_embedded_tauri` 是唯一 React UI 宿主；`linux_server_api` 只提供管理 API 与健康检查，不提供网页、登录或静态资源。组件依次调用 query/domain、类型化 transport 和 `NexusHubUseCases`；Provider 读取与受控写操作位于共享核心，HTTP/Tauri 只做薄适配。

| 模块 | 职责 |
| --- | --- |
| Codex | 原生 state/index/rollout/log 读取与身份核验，受保护的管理操作，以及本机主线程的原生桌面协作 |
| Claude Code | 递归发现、分支历史、原生进程身份、受保护的改名和单会话删除 |
| Grok Build | 原生目录发现、主回合历史、登记与进程校验、改名及定向删除 |
| Probe | 错误与终态识别、游标、基线、去重、脱敏、保留策略和加密 Bark 投递 |
| NexusHub 数据库与配置 | 自有设置、任务、事件、投递、加密发送队列及迁移；不包含原生 Provider 数据迁移 |
| WebUI | 展示契约、查询状态、执行分组、搜索、附件、Plan 导出及本机 Codex 协作输入区 |
| 打包 | macOS ARM64 Tauri App/updater 与 Linux x86_64 无界面 webd |

主窗口由 `desktop_boot` 隐藏创建，只在初始化时最大化一次或使用工作区回退，Ready 显示；Reopen 只显示、恢复和聚焦，不重新设置几何尺寸。

## 共享契约与修改流程（Shared changes）

API 协议固定为 **2**，`system.capabilities` 返回协议、宿主和能力矩阵。产品版本、协议版本和契约文件的 `schemaVersion` 是不同概念，不互相替代。

1. 同步 `contracts/nexushub-contract.json` 及其 schema，明确 action、DTO 归属和宿主能力。
2. 同步 Rust 核心 DTO/use case 与 Linux/Tauri 适配层。
3. 同步 WebUI 的类型、query/domain 和对应测试。
4. 执行契约、Rust、WebUI、安装及隐私门禁；准确命令见 [README 开发](../README.md#开发)。

共享动作必须在 registry 中有明确映射；桌面专属 `remote.*`、`plans.save`、`codex.desktop.*` 不进入 Linux dispatcher。旧命令及别名集中为 tombstone，保留原适配层的不可用结果，不重新启用执行入口。新增类型或可选字段必须同步真实序列化结构，不能仅增加同名标记。

`contracts/wire-types.json` 是测试用映射及审阅后的源结构清单：Rust 用已有 syn/quote 开发依赖读取实际类型、serde 属性、枚举、适配签名及动态 JSON 表达式；TypeScript 用锁定编译器 API 读取实际声明及调用包装。自由 JSON 与内联请求必须明确归类；代表性真实序列化测试验证缺省、null 和枚举拼写。仅名称标记不能通过实际类型/注册函数检查，负面 fixture 覆盖缺字段、缺映射和缺适配入口。更新清单必须同时审阅线上 wire shape，不能用重新生成清单代替兼容性判断。

## 本机 Codex 桌面协作

`codex_desktop` 能力仅属于 macOS 的 `desktop_embedded_tauri`。八个 `codex.desktop.*` 桌面命令经类型化 query/API 和 Tauri 适配进入 `codex::desktop`；Linux dispatcher、远程 allowlist、原有 RPC 与 API 协议 2 保持原边界。旧服务没有该可选能力时继续阅读。归档、不可核验、远程来源和子线程不能执行。

IPC 通过私有 Unix socket 连接原生桌面，校验同 UID、文件身份、权限和 peer；逐次发现 owner，跟随精确本机线程。长度前缀帧上限 64MiB，连接/请求等待 10 秒；状态快照与补丁严格检查修订号、路径和预算，失步关闭。每个 App 最多四个会话 worker；只按需释放无队列的空闲 worker。独立的短生命周期 bundled helper 只读取模型/模式/技能/应用/插件目录或创建空线程，不执行 turn、resume 或队列；helper 串行门禁最多等待 20 秒，取得许可后每次原生操作另限 20 秒；多页目录并非整体只等待 20 秒。退出或取消时仅回收自有 helper。

原生执行使用 `desktop/protocol.rs` 中固定的白名单，依据桌面 build 20586：stream=11、queue=2，start=2、steer=1、interrupt=4、settings=2、审批/问答/elicitation=1；initialize=0、owner discovery/follow=1。没有猜测或探测新的写入方法。协议不支持、owner 不可用、修订失步、超时分别返回固定中文提示；前三种情况同时提示已核验 build 20586 和在桌面打开同一线程后重连，不探测新的协议；历史阅读仍可使用，不启用其他执行器。发送前重验线程存在、未归档、当前 owner 与状态；补充/停止绑定当前回合。NexusHub 自有发送队列独立于原生整表队列，不覆盖原生排队内容。只在已核验空闲状态时发送，并等待返回的确切回合终态才发送下一项；原生界面并发操作仍由原生桌面处理，IPC 不提供跨界面的原子空闲锁。

自有数据库 schema 2 增加加密发送记录，schema 1 退役迁移保持独立完成标记。专属 Keychain 密钥通过 `CredentialAccess` 访问，没有开发密钥回退；缺失密钥时保留已有密文。跨进程私有文件锁限制唯一调度者。认领先落盘，只有原生确认或匹配 client message ID 后清除正文；断线/重启的未发项暂停，正在发送项标记结果不明，不自动重发。普通补充失败保留待取消，不能在恢复队列时改成新回合。新增非空引用区间以已解析引用索引保存，不依赖会过期的界面 handle；没有区间时沿用旧加密正文的逐字节格式，旧 client message ID 重试仍按原正文判等。停止先暂停本线程未发队列，移除结果不明记录需要明确核对。

单条加密正文最多 1MiB，总密文 32MiB，最多 1,000 个未结项、每线程 50 项，已结束身份保留 1,000 项；限制覆盖保留内容，不代表进程 RSS 上界。目录投影缓存最多八个快照/8MiB、每快照 20,000 项，60 秒失效，每页 100 项；选择授权最多 256 个、30 分钟，绑定连接 revision 与线程。缓存锁不跨越 helper I/O，同来源、线程和目录种类的并发读取合并；失败只共享给当轮等待者，不缓存。模型设置同样通过新鲜目录验证。失效 cursor 不能继续使用，界面重试会从首页开始；应用和插件串行获取，单个来源失败不丢弃另一个成功来源，每次最多挂载 100 项。

WebUI 仅传不透明 handle，`referenceSpans` 为协议 2 内的可选字段，包含 handle 与用户文本中的 UTF-8 半开区间。输入 `$` / `@` 后可用上下键、Enter/Tab 选取技能或应用/插件，Escape 关闭；输入法组合时不选择或发送。编辑占位文本会撤销相应绑定，删除标签会同时移除其占位文本。Rust 从授权引用推导占位文字，校验身份、名称、字符边界、非重叠和最多 32 个区间，组装原生 `text_elements[].byteRange` / `placeholder`，同时保留原有结构化 skill / mention；无区间仍显式发送 `text_elements: []`。插件使用原生 canonical name 插入，友好标题保留为目录说明，避免显示名与原生引用不一致。

原生选择器路径和原生目录引用仅在 Rust 解析；选中文件及队列发送前核验规范路径、dev/inode、大小与修改身份。图片限 20MiB，仍使用 `localImage` 和既有附件字段；Mention 使用 `app://id` 和 `plugin://name@marketplace`，仅展示原生可访问应用与已安装启用插件。普通文件暂保留 `# Files mentioned by the user:` 兼容前缀；如同时有内联引用，Rust 在构造完整文本时统一平移区间。

`codex.desktop.attachments` 只处理本机主窗口的接收区绑定、坐标更新、明确粘贴、草稿引用释放和解绑；请求只含 revision、session/binding、矩形与 handle，不含路径、URL、图片字节或原生方法名。Rust 接收 OS 拖放，核对窗口可见/聚焦、区域、当前来源和可写线程后复用文件选择核验，整批超限或含无效文件时整批拒绝。单窗口和多 WebView 的原生拖放路由分别处理；禁用通用前端 event listen 权限，防止 Tauri 默认拖放事件把原生路径送入 JS，只用专用 Channel 返回名称、种类和 handle。发送或选择文件期间暂停接收；隐藏、切换与解绑后丢弃迟到导入，普通文本粘贴保持原行为。

粘贴只在明确的图片 paste 事件后读取系统 PNG/TIFF，不读取 HTML、文件 URL 或网络图片。单次捕获最多 64MiB，解码前按尺寸预留每像素 16 字节、共 128MiB 工作预算；只接受单图且深度不超过 16 位，输出 PNG 最多 20MiB，不缩放或降低质量。PNG 原样保留，TIFF 由系统框架无损转换；读取及转换后均核对 clipboard changeCount。导入串行，预算不代表系统剪贴板服务或进程 RSS 的硬上界。

粘贴图片写入 NexusHub 数据库旁的私有附件目录（目录 0700、文件 0600、随机名、独占创建），写完并同步后捕获原有 `NativeReference::File` 身份，之后不改名或改写。图片为原生 owner 可读取的明文私有文件；归属和生命周期索引使用现有专属 Keychain 密钥加密，索引上限 1MiB，图片总量最多 512MiB/256 项。沿用跨进程 worker 锁，初始化/恢复后台工作取消也不提前释放锁。未提交草稿可显式释放或在 30 分钟后清理；隐藏/重连在授权有效期内保留草稿。发送前先原子持久化并同步保留标记，再交给原有队列，成功、失败、取消或结果不明均不自动删除这些图片，避免破坏原生历史。容量满时拒绝新粘贴，可继续使用文件选择器。启动只清理有完整归属证据的过期草稿，`Released` 标记允许接续草稿删除；密钥缺失、损坏/缺失索引、未知遗留文件或身份变化时保留文件并停止操作，不猜测崩溃后的归属，不新增数据库表或旧队列格式。

审批 key 包含完整原生请求；文件审批另外绑定对应 turn/item 的实际 changes。展示不完整、未知权限/表单约束均不得批准，保留原生处理入口。命令与文件审批在 `supportedDecisions` 可选字段明确声明后，才显示“允许本次会话”；新增可选 `decision` 保留原 `allow` 布尔兼容，只接受已核验的 accept/decline/acceptForSession 及一致布尔值。未知决策为 UnsupportedProtocol，过期 key 为 RequestExpired；权限仍限 turn scope，不创建永久授权。问答只提交当前请求的精确 ID；原生另一侧完成请求后以新的原生状态撤下原请求。

审批的可选 `review` 字段按已有字符串和结构投影命令、工作目录、原因、文件变化与网络/文件系统权限；旧 `body` 后备仍保留。未知字段放入折叠原文，命令用等宽文本、文件 diff 按文件折叠且展开才挂载；每个 diff 最多 32KiB、最多 100 文件、投影文本总量 64KiB，UTF-8 边界安全截断。原有 pretty body 16KiB/compact params 64KiB 审批守卫保持；截断或未完整显示时不能批准。序列化与请求哈希使用有界写入或流式处理，不克隆整份 params/changes，不改变完整请求绑定的哈希。

协作状态直接取当前 `DesktopView`：等待请求优先于执行、发送、排队、结果不明/暂停和空闲；同一行显示待处理数，运行复用 `RunningIndicator`。断连、只读、隐藏或 detach 后不显示旧状态，不添加 IPC 方法或未知进度字段。

Tauri 频道最多每秒十次投影视图，初值取自同一订阅；前端丢弃旧修订、旧连接或旧线程结果。仅当前协作线程通过 350ms 合并事件刷新详情；慢读取不被反复取消，期间的新事件在完成后触发一次尾刷新，包含审批完成和回合终态。原有阅读轮询保持，未添加全局高频轮询。隐藏/离开页面释放界面订阅和待刷新计时器，已提交的本机队列继续归原 worker；原有 monitor 和通知仍独立读取原生证据。

### 证据与未核实项

1.3.1 的只读核对依据桌面 build 20586 应用源码；`crates/nexushub-core/tests/fixtures/desktop-native-input/source-derived.json` 保存源文件标识、SHA-256 和脱敏合成输入，**不是桌面采集快照**。`conversation-turn-mutations` 确认内联字段为 camelCase `byteRange`；审批面板的 Qr / $r 与 follower 转发实现共同确认命令和文件的 `acceptForSession`。未复制原生源码到运行时代码，也未采集真实用户消息。

- **【未核实】** 桌面原生文件引用的完整 wire 形状。1.3.2 未采集 liveCaptured snapshot，N0/N1 按批准范围跳过；因此普通文件兼容前缀和旧加密正文保持，不宣称与原生气泡完全一致。
- **【未核实】** 文件审批 `cancel`、永久授权、策略修订和其他未列举决策；未暴露按钮或发送这些字符串。
- **【未核实】** 本轮真实 skill/app/plugin/文件/图片 snapshot、两端操作及视觉/滚动体验；合成 socket 与 UI 单元测试不替代用户验收。
- **【未核实】** 仅 IPC pending、rollout 未落盘时的漏通知。当前 monitor 依据原生 rollout，尚无可复现的漏通知证据，N6 不扩展 worker 通知，也不声称已经证明无缺口。
- **【未核实】** 其他桌面 build 的兼容性；版本或状态不匹配时只读降级，由用户在 Codex 中继续处理。

## Provider 读取与历史

### 共享读取模型

`jsonl_cache` 核对文件身份、大小、mtime/ctime 和已消费前后锚点。冷读流式消费，普通追加只解析新增完整记录，半行等待写完。各来源独立串行；解析、分页不持有全局缓存锁，未变化结果复用不可变快照。列表、详情、搜索和子智能体复用快照；管理操作独立重新核验来源。

`read_tasks` 使用持有至真实结束的许可，把耗时读取移入有界后台，避免阻塞 Tokio 执行线程。摘要缓存保留服务端发现的身份，使用前重新校验；macOS/Linux 的派生摘要复用 dev/ino、大小、mtime/ctime，其他缓存保留边缘校验。带附件响应按需复制、重新检查附件，后续追加不能改写旧响应。

`history` 包含 epoch、revision 与有界变更记录。来源替换、截断或分支改变使旧窗口失效。Codex 保留 `b:index`，支持附带块 ID 的位置提示及 `a:block-id`；现有 `before` 同时支持 `j:` 定位、`p:` 向前、`n:` 向后、`w:` 刷新窗口。Grok 的 limit/before 可选，省略时保持原完整响应。

### Codex 与子智能体

详情直接按原生 ID 定位。完成工具块保持原顺序，结果与调用配对并放在调用位置，历史压缩保留工具分隔。`history_navigation` 在分页和压缩前建立完整用户消息索引：重复文本仍保留独立原生身份，纯自动上下文不进入索引。最新详情可返回 `timeline`，旧窗口不重复传输；定位窗口使用未压缩块。

`MessageBlock.subagent` 通过原生调用 ID 配对创建与结果，再按线程 ID、精确任务路径和父子图关联；不按昵称猜测。`threads.subagentDetail` 从可见主线程检查唯一父链、循环、深度、索引、rollout 身份、符号链接及读取前后指纹，不接受客户端路径。子线程历史独立分页，附件以 `rootThreadId` 绑定相同关系验证。`subagent_updates` 可更新已加载但不在最新页的卡片。

`item_completed → SubAgentActivity` 的 started/completed/interrupted/interacted 表示历史活动，不能用外层完成推断子线程终态。同次创建、结果及开始事件去重，续接的开始时点保持独立。`eventKind`、`eventId` 保留原始活动身份；任务名按 task_name、已验证路径末段、标题和昵称的顺序取得。

`ThreadDetail.subagents` 在分页前按直属原生线程 ID 去重，包含状态计数、完整性及逐项不可用原因，不累计孙级或回合数。当前状态只依据子线程自身回合和未完成工具，父文件未变也需刷新；存在运行中或创建中子线程时采用活动轮询频率。UI 可省略通用完整性说明，但不得改变 complete/warning、关联和未知状态语义。

### Claude Code

opaque session key 绑定相对 transcript 位置和原生 ID，同名 ID 互不混淆；子智能体文件不作为独立主会话。详情按稳定事件身份分页，保留分支和原生事件顺序。常见 2.x user/assistant、标题、工具、queue/history/usage、compaction 等记录可读；未知记录仍显示，格式未验证、损坏或半行记录阻止管理。

运行显示要求已识别进程、启动时间、会话/项目匹配及未结束回合。管理更保守地拒绝任何潜在占用。改名仅追加已验证的 `custom-title` 并确认持久化；删除只隔离和复核目标 JSONL，不改原生索引、配置、安装或项目工作目录。

### Grok Build

活动快照用一轮进程表校验登记数组/对象、原生可执行文件身份与进程启动时间。退出、僵尸或复用 PID 不能证明仍运行。只有活跃归属与未结束主回合同时成立才显示运行；不可读、损坏和身份不确定保持未知。

冷读消费完整历史，不截取尾部；增量缓存保留文件身份、ctime 和前缀锚点。可选 `deletionEligibility` 与活动显示分离：没有经验证原生占用、记录完整有效时，缺少历史结束事件仍可预览；登记或身份不明、损坏和未完成写入继续保护。列表和批量预览复用进程快照，执行及隔离后重新采集占用并核验指纹，包括仍打开但闲置的会话。

## 消息、搜索与界面状态

用户消息与助手 Markdown 分开处理，保留用户字面文本和换行。三个 Provider 的原生消息/图片分组形成稳定 user message；共享投影拆出正文、附件及可选 `contextBlocks`，类别固定为 instructions/environment/runtime/files。只在代码和引用之外识别完整白名单包装；允许相邻标签和完整 INSTRUCTIONS 的同行结束标签，内部标题不截断包装。原生问答回复、用户 shell 操作和中断保留专用解析。

新旧 `My request:` / `My request for Codex:` 附件包装均可识别；按原生身份去重并优先已有嵌入图片，失效临时文件不隐藏请求。消息按附件、一个默认收起的“上下文”、正文排列；上下文展开才挂载，纯上下文不产生空气泡或时间线刻度。展开原文按原顺序保留，全部清理仅用于展示，不回写会话。

`visibleMarkdown` 在解析前按源范围去除结构化 memory 与完整 `external_codex_apps_open_page`/page_id 元数据，并有渲染保护；CommonMark 代码、引用和普通示例不被误删。显示、复制、预览、Plan 内容和文件名共用清理结果。指令分段及完整 Markdown 转换缓存不保存 React owner、路径解析结果或折叠状态，每次恢复独立语法树后继续执行链接安全处理。

`sessions.search` 是当前机器内的只读动作，输入 Provider、线程/Provider 范围、opaque key、query、cursor 和 limit，不接受文件路径。搜索摘要与有界正文页，返回稳定 block/event position key。用户搜索取实际正文、问题/回答、附件名和指令块，自动字段不重复命中。AGENTS.md 命中仍能展开上下文。Plan、工具、助手回复保留搜索锚点，但不成为用户时间线刻度。

`usePagedHistory` 分离最新窗口轮询与历史取页；静态页不重复请求，只刷新变化的已加载身份，快速追加时补齐中间窗口。`historyScrollCoordinator` 统一首次底部定位、上滚、搜索分页、请求互斥和可见锚点补偿；失败或游标不前进停止自动重试。`historyWindows` 保留最早游标；`useTimelineWindow` 直接取定位窗口，复用最新变更记录，无需下载中间全部历史。

搜索通过 `DisclosureRevealContext` 挂载命中折叠内容后定位，优先精确原生 ID，其次最深别名节点；Codex 压缩只豁免当前机器/线程最近命中的一个块，不展开整段历史。时间线位置只在布局变化时重测，刻度有界挂载且键盘可达全部记录。工作区按需加载，隐藏窗口暂停界面轮询，后台 monitor 独立运行。

显式折叠选择按机器、Provider、会话、活动身份保存在有界内存中，不写数据库。机器、线程或来源变化取消无效请求并丢弃迟到响应；子智能体面板有独立分页、滚动与嵌套返回，父视图保持挂载。连续工具组保持原位置，完成默认收起，活动/失败默认展开，指令组始终默认收起；用户选择优先于默认值。

### 附件、路径与 Plan

`sessions.attachmentRead` 经共享 use case、身份校验和薄适配读取；只接受 Provider、session/message/attachment ID，以及子线程必要的根线程上下文。重新解析来源及文件身份，PNG/JPEG/WebP/GIF 单张预览限 20MiB。视口附近才读取图片，前端缓存按机器隔离并在切换时清除；轮询只携描述，不含图片，不抓取仅有远程引用的附件。

文件链接在浏览器解析 URL 前从原始 Markdown target 提取，剥离行列后缀并按线程 cwd 解析相对路径。远程只复制，本机 macOS 通过 Tauri opener 在 Finder 定位。`plans.save` 始终是本地 Downloads 导出，校验有界 Markdown 文件名/正文并独占创建，不覆盖既有下载、不经过远程 HTTP。

## 管理安全与隐藏扫描

批量请求只接受明确 Provider keys，预览和执行逐项返回结果；筛选变化清空选择，轮询不得追加选择。Codex 普通/归档删除以预览指纹绑定身份、明确终态、数据库/索引关联、共享引用和文件内容；先进入私有隔离目录，再配合数据库事务与索引原子替换。普通错误尝试恢复文件和索引，回滚不完整时保留可恢复数据，不级联删除未选中线程。强制终止后的隔离数据保留不等于启动时自动恢复。

`cleanup.hiddenDryRun` 固定扫描开始时的候选，集中读取一次数据库元数据、会话索引和原生目录，建立引用映射；逐项流式核验终态、身份和完整指纹。取消检查间隔最多 64KiB。可选 start/poll/cancel、扫描 ID 与游标复用原 action；无参数保持原完整响应，重复游标返回相同已发布结果，扫描中发布完整页，完成/停止发布尾页。

扫描每页最多 100 项，同源最多一个活动扫描，进程最多两个上下文，共用 64MiB 预算；连续五分钟无请求取消释放。前端顺序取页、分页显示结果，完整快照才允许确认清理；停止保留已显示结果但禁用执行，切换机器隔离迟到响应。

`cleanup.hiddenExecute.candidates` 是执行边界，`expectedCount` 比较候选长度而非全局隐藏数；旧无候选请求不能执行。每项重新核验后用 savepoint 执行，项目级错误跳过并继续，公共数据库/索引/存储故障回滚整批；提交前复核暂存与数据库完整性，不加入预览后新增的线程。Linux/Tauri 共用有界后台，不改原生登记或终止用户进程解除保护。

## 远程连接与认证

React 只向原生 IPC 传命令、参数及连接 revision。Rust 固定 allowlist、HTTPS-only、禁止重定向，读取响应累计上限 32MiB，足以承载上述 20MiB 图片的 base64。首次使用先验证协议和宿主能力。Key 仅在钥匙串与 Rust HTTP 层使用，不返回 React、不进入 URL 或日志。

连接 revision 隔离 QueryClient、工作区生命周期、预览、选择、分页、滚动、标题覆盖、附件和折叠；写操作不自动重放。当前机器拥有唯一更新面板和任务状态，本机使用 Tauri updater，远程使用服务 RPC；失败不暗中切回本机。

`CredentialAccess` 以一个信号量许可串行后台凭据操作。读取等待上限 15 秒覆盖排队和执行；已进入原生 API 的工作不能取消，超时/取消仍持有许可至真实结束。尚未开始的阻塞任务可取消。保存/移除只在等待许可时允许超时，事务开始后等待提交或回滚；`ChangeGuard` 随后台事务保留切换保护。读取 Key 和验证能力后再次核对 revision、目标与地址，回写也必须匹配；错误不返回原生异常正文。

`ServerConfig` 在配置加载及绑定前验证回环监听；允许 IPv4 127.0.0.0/8、IPv6 ::1，IPv4 映射地址按其 IPv4 值判断。拒绝发生于数据库打开、后台任务启动及配置迁移写回之前，错误不回显输入；原有默认端口和旧回环端口迁移保留。

服务端只接受一个 `x-api-key`，保存 SHA-256 摘要并常量时间比较；失败限流与审计不含凭据，合法 Key 不被失败尝试锁死。来源 IP 是现有安全审计内容。CLI 轮换立即使旧 Key 失效，撤销拒绝所有业务 RPC。systemd 保持文件系统隔离，只为已配置 Provider 根开放精确 `ReadWritePaths`，缺少 Claude 目录不阻止启动。

更新任务与 Probe helper 使用内部 `ProcessCommand` 的 program/args 直接启动，不执行配置 shell 字符串。现行和完整匹配的历史默认更新命令映射到固定包装器及参数；自定义 shell、环境赋值、管道、重定向和命令替换返回固定错误。任务名称、状态、输出处理、互斥与审计字段保留；备份清理直接运行固定 Python 参数，保留最近三份。序列化 command 字段仅保留兼容展示，不改变 wire shape；Hook/配置文本的 shell_quote 统一实现并保留既有引用策略。

## 通知与监测

事件识别与 Bark HTTP 投递分离。Codex 错误绑定规范主任务身份；普通自然语言反馈仅在确认终态后本地识别，monitor、Stop Hook 和完成入口共享 `assistant_question` 与持久化认领。Grok 使用主回合结束证据；Claude 要求明确终态且无 pending tools，空完成内容回退到本回合最后一段助手回复。兼容警告继续可读并保护管理，仅致命解析/身份错误禁止扫描。

原生 Codex 问题解析位于 `codex/user_input.rs`，统一同步 question、异步 title 和字符串/对象选项。tracker 以原始 turn/call/question 身份配对完整回复，处理明确取消与替换；工具确认、背景进度或正常完成不能关闭尚未回答的问题。异步 monitor 独立于列表状态，与 Hook、passive、Stop 共用发送前复核。启用基线和原子 per-call 认领保证首次跳过旧历史、部分回答不产生新身份，重启/TTL 过期不重发已确认调用。

Claude 通知复用 provider monitor 的游标和原子认领；仅明确完成/失败、未解决 AskUserQuestion 或绑定 pending 主工具的权限请求符合条件。不从普通待完成工具猜权限，不使用 Codex 自然语言推断；原生记录未保存的权限不能通知。首次启用和兼容读取器升级建立基线，新请求取代未发送旧事件，发送时复核身份和 pending 状态。

Bark 保存和发送前共用 URL 校验。HTTPS 拒绝回环、未指定、链路本地、IPv4 私网、IPv6 ULA 及对应映射地址；拒绝数字 IP 别名、非法 authority 和 URL 凭据。保留普通 HTTPS 主机名与真实 HTTP 回环服务，不作 DNS 解析。无效配置保留原值并停止投递，不改用默认服务携带原密钥发送。

`bark` 复用支持 HTTP/2 的连接池并禁重定向，分类 DNS、连接、TLS、超时、读取、HTTP 及业务拒绝；已知永久 APNs 错误优先。原始异常仅用于内部分类，对外保留安全分类、状态码、耗时、次数及匿名关联，不输出 Key 或正文。

`notification_delivery` 将不可变请求和分段加密入队，事务化认领/确认/事件回写；租约持有者才能确认，过期租约只恢复未确认分段。每分段最多 3 次，等待 15/60 秒并加 ±20% 抖动，首次投递后最长 10 分钟；尊重 Retry-After，超出剩余期限即终止。成功分段不重发，稳定通知 ID 由机器实例、事件与分段组成。

Hook 短等待，后台每轮最多领取 10 项；重试在来源验证前后检查当前配置、身份、开关、目标与事件有效性。正常追加以原始前缀和文件身份复核，不因长度变化取消。结束清除队列正文，同一 UI 事件更新状态。旧终态失败/未知不回放，新网络未知可有限重试且可能重复提醒。主动测试使用独立身份和同一队列，最终未受理以失败退出；受理不代表手机实收。

## 大小统计与资源预算

`session_storage` 只统计已验证归属的逻辑字节，不读正文或计算删除指纹。Codex 统计 rollout，Claude 统计主 JSONL 及同项目内确认身份的同名会话目录，Grok 统计专属目录；独立线程、共享数据、工作目录和外部附件不计入，不跟随符号链接。

三个摘要的可选 `storageSize` 使用 bytes/scope/status，未知不等于零。普通追加可复用身份缓存；截断、替换和归属改变立即失效。K/M/G 按 1024 换算，颜色按准确字节：小于 100MiB 绿色，100MiB 至不足 300MiB 琥珀色，至少 300MiB 红色。未知灰色，部分结果按确认下限分色并保留 ≥，提示准确字节和不完整状态。

固定预算由 Rust `resource_limits` 和 WebUI `resourceBudgets` 命名管理，不是设置项。每个 JSONL/普通读取缓存默认最多 64 个来源，以下字节数独立计重：

| 缓存或工作 | 当前预算 |
| --- | --- |
| Codex 正文 / 活动扫描 / session index | 384 / 8 / 16MiB |
| Codex 问题 / 子线程生命周期 / 关联 | 4 / 4 / 1MiB |
| Claude 正文 / 摘要 | 512MiB；摘要 2,000 条、8MiB |
| Claude 输入文件 / 单条记录 | 512 / 32MiB |
| Grok 正文 / 活动 | 192 / 2MiB |
| 原生通知派生读取 | 32MiB |
| 历史变更记录 | 64 个 revision，合计 4,096 个变化身份；超限要求安全刷新 |
| 大小缓存 / 队列 / worker | 512 项 / 128 项 / 2 个；TTL 30 秒 |
| 单次目录统计 | 10,000 项、16 层、250ms |
| 阻塞读取 / 凭据工作 | 按可用并行度限制为 2–4 个 / 1 个 |
| WebView 指令切片 / 上下文投影 / Markdown 树 | 各 512 项、各 8MiB，分别计重 |

上述读取模型的配置预算合计 1,163MiB，但**不是进程内存硬上界**：正在引用的 Arc 快照可能暂时不可淘汰，解析、响应序列化、恢复的语法树、DOM 与并发请求有额外内存；App、helper、远程服务和 WebView 又是独立进程。目录条目按数目而非完整堆内存计重，隐藏扫描预算见管理章节。超大快照可返回但不长期保留；不能以改变成功读取行为的方式把配置预算解释成全局 RSS 限额。

## 兼容与自有数据迁移

最低兼容边界继续由协议 2、宿主及能力响应判断，不仅看版本字符串。下表的版本表示字段/能力首次随 Release 提供，不宣称旧服务拥有后来新增的能力。

| 能力引入 | 缺失时的现有行为 | 未来删除回退的前提 |
| --- | --- | --- |
| 1.2.11 直属集合 | 保留既有阅读和摘要状态回退，缺能力按不支持处理 | 最低服务具备对应集合和能力 |
| 1.2.16 storageSize | 显示未知，不能当作零 | 最低服务保证大小 DTO |
| 1.2.17 history/稳定 cursor/Grok 分页 | 旧 cursor、全量快照或逐页刷新 | 最低服务保证变更元数据及分页 |
| 1.2.20 hidden_cleanup_scan/deletionEligibility | 单次扫描无虚假进度/取消；Grok 使用原活动资格判断 | 最低服务保证两项能力 |
| 1.2.21 contextBlocks/timeline | 保守解析原包装；历史顺序加载和定位 | 最低服务保证结构化内容及索引 |

不删除 tombstone、回退或有效迁移。tombstone 至少保留整个协议 2 生命周期；移除回退需明确提高最低服务版本；数据库迁移按历史数据库升级支持范围保留，不能仅因服务版本提高而移除。

迁移只打开 NexusHub 自有数据库。schema 版本标记成功完成的升级，重复打开跳过退役 JSON 扫描；跨进程迁移锁覆盖含 VACUUM 的全过程。完成所有迁移后才推进版本，失败保留原有 pending 标记，下次可重入；退役阶段不新增业务表或用户设置。桌面发送记录通过后续 schema 2 独立建立，不重复退役扫描。

退役范围限于 Goal 表/恢复列、旧网页 admins/sessions/Turnstile、Pi 自有提供方/流/投递/事件/设置，以及 Gotify 配置、密钥、通道、队列、Probe 字段和专属任务。Pi 队列先于 JSON 解码清理；涉及退役秘密时使用 secure_delete、VACUUM 和 WAL 截断，保留 Bark 密钥、成功/未知记录、基线、任务、业务审计及其他设置。配置迁移只删明确退役键，不卸载外部服务，不打开或迁移原生会话数据库。

## 构建、CI 与发布

根 Cargo workspace 构建共享核心和 Linux webd；`src-tauri` 独立 workspace 避免服务端构建引入桌面平台依赖，路径依赖共享核心。两份锁文件独立解析，Tauri 自身允许额外主版本；共享依赖约束、实际选择和最低 Rust 要求需一起检查。核心与桌面最低 Rust 声明为 1.88；开发与 CI 由 rust-toolchain.toml 固定为 1.99.0，CI 另用 1.88.0 检查受支持的 Linux 服务与 macOS 桌面目标。`check-workspaces.py` 精确比较共同直接依赖的声明与锁定结果；当前允许的 `toml` 0.8 解析差异是根 workspace 0.8.23、桌面 0.8.2。Tauri 额外的 dirs、reqwest、toml 主版本属于独立依赖链，不能据此强行统一。

升级共享依赖时先确认两侧 manifest、feature 和实际依赖链，再定向更新两份锁文件、运行两侧 Rust 检查和三项 CI；不得为消除合法多版本而全量更新。产品版本以根 Cargo workspace 为准，JSON/Tauri manifest 和两份 lock 的自身 package 必须精确一致；契约 schemaVersion 与 API 协议单独校验。

CI 保留 Frontend checks、Backend checks、macOS Tauri checks。第三方 Action 固定 SHA，权限按作业收敛；Frontend/Backend/Tauri 超时为 30/45/60 分钟。Cargo test、clippy 和正式构建均使用 --locked，包括 Tauri 传递的 Cargo 参数。缓存区分平台、架构、工具版本、锁文件与构建参数，覆盖 Cargo、pnpm、固定版本 cargo-audit 和 Playwright 浏览器；Linux 浏览器系统依赖仍正常安装。cargo audit/pnpm audit 保留 JSON artifact：高危、严重及无法判级的活动漏洞阻断，低/中危和 RustSec informational 警告进入 summary；扫描失败、数据库获取失败或报告不完整也阻断。定向修复必须审阅两份锁文件，不自动批量升级。交互性能预算使用生产构建、可见 macOS 窗口、串行 Chromium/WebKit 和关闭 trace 的命令；单一阈值与 JSON 测量结果由既有 `@desktop-performance` 测试提供，复现见 README。

Release guard 核对 tag/版本、契约、隐私和同提交三项成功 CI。guard/Linux/macOS/publish 超时为 10/45/60/15 分钟。发布仅有 macOS ARM64、Linux x86_64 webd 和汇总阶段，共八项资产：DMG/校验文件、updater 归档/签名、`latest.json`、`nexushub-webd-linux-x86_64.tar.gz`/校验文件/分离签名。updater 只映射 `darwin-aarch64`，文件名的 darwin-arm64 是同一架构；webd 不能成为桌面 updater 条目。

正式安装复用同提交成品，先远程 API 后 App；部署输入不进入仓库。Linux 归档用现有 Tauri 签名工具生成带版本 trusted comment 的分离签名，私钥仅从 GitHub Secrets 环境提供。在线下载先校验发布 SHA-256，再用预装 Minisign 与固定公钥验签，失败不解包；指定版本需与已签名版本相同，解包后核对布局、ELF x86_64 平台及程序版本。旧 updater 首次升级由可信客户端先验签；显式本地 --archive/--binary 运维方式保留。部署另比对本地与远端文件 SHA-256。解包拒绝路径穿越、链接、特殊文件及重复成员；目录与可执行文件统一 0755，普通文件 0644，不继承特殊位或可写权限。程序与部署文件通过同文件系统临时文件原子替换，普通安装失败恢复本轮替换项，不回滚业务数据库或原生会话；在线更新仍使用既有异步重启，成功调度不等于新进程已启动，也不提供崩溃自动恢复。安装成功/启动检查与用户功能验收分开报告。维护六份当前文档并使用普通增量提交，历史由 Git 和 Release 保留。
