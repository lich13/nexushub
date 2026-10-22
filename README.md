# NexusHub

NexusHub 是一个会话阅读与管理工具，用于在 macOS App 中查看 Codex、Claude Code 和 Grok Build 的本机会话，也可以通过受保护的 API 管理一台远程机器。本机 Codex 主线程支持连接原生桌面继续协作。

## 主要能力

- 按 Provider 搜索会话、搜索当前线程或全部线程，并用时间线定位消息、Plan、附件和工具活动；时间线短线只显示识别到的用户指令。
- 保留文本与工具的原始顺序；连续工具活动可以展开查看。Claude 同一回合的思考、临时说明和工具活动合并为默认收起的过程折叠，最终答复单独显示；自动上下文合并为一行默认收起，真实请求与附件正常显示。
- 复制线程 ID、回复和 Plan；Plan 可保存为 Markdown，文件路径可以复制，macOS 本地路径可以在 Finder 中定位。
- 三种 Provider 共用 ⌘ 点击多选、Shift 区间追加和 100 项上限；双击可改名。Codex 支持归档、恢复和整树删除，Claude Code 与 Grok 提供受保护的改名和删除。
- 通过 Bark 接收完成、需要回复、需要授权和失败的简洁摘要；本机 Claude hooks 可在设置中明确启用，与轮询共用去重，Codex 保留逐问题提醒。
- 在本机 Codex 新建或继续主线程，发送消息、排队、补充与停止；选择原生模型、思考强度、默认或 Plan 模式，添加文件、图片、技能、应用和插件引用，并处理受支持的审批与问答。

对话和工具始终由原生 Codex 桌面执行；NexusHub 不独立运行模型或安装 Provider。远程、归档及子线程，以及 Claude Code、Grok 的对话仍只读。阅读和展示不会改写原生会话；改名、归档、恢复及删除只在用户确认的范围内执行。

## 本机 Codex 协作

打开本机 Codex 主线程，在协作区选择“在 Codex 中打开”，等待原生桌面打开对应线程并自动连接。“连接”用于连接桌面已打开的线程，本身不会打开目标线程；仅在 NexusHub 中选中历史线程还不满足这个前提。新建时先选择工作目录，空线程打开后也会自动连接；尚未发送消息时没有对话内容。普通浏览线程不会启动协作。需要已安装并运行兼容的 Codex 桌面；以实际协议和线程归属判断兼容，不按 build 号限制发送或显示常驻提示。确实不兼容时保留阅读，并提示在 Codex 中处理；未来破坏性协议变更仍可能需要适配。

输入后按发送按钮或 ⌘/Ctrl+Enter。运行时“排队”留作下一回合，“补充”发给当前回合，“停止”中断当前回合并暂停本线程待发送队列。待发送消息与引用元数据在本机加密保存，断线或重启后由你确认继续；结果不明的记录需先在 Codex 核对，不会自动重发。队列与原生桌面自己的排队内容分别保留。

短暂断线会尝试恢复；桌面重启导致线程 owner 改变时只恢复阅读，点击“连接”确认后再继续协作，暂停队列仍需手动继续。提交期间会显示“正在提交到 Codex…”。遇到问题可点击“复制诊断”，即使连接失败也可取得安全错误分类和版本信息；不包含聊天正文、路径或凭据，复制完成会显示“诊断已复制”。

文件和图片可以由原生选择器选择，也可拖入协作输入区。从 Finder 复制文件后可直接粘贴；每行一个完整的绝对路径、`~/` 路径或本机文件 URL 也可自动附加，路径允许空格及成对引号。只有整段内容都是有效文件时才附加，普通正文或失效路径保留为文本。截图及 PNG/TIFF/JPEG 图片支持直接粘贴，Finder 文件优先于预览缩略图。粘贴图片保存在仅当前用户可访问的本机私有文件中，供原生桌面读取；交给发送队列后会保留，避免影响历史图片。存储满时可改用文件选择器。

输入 `$` 搜索技能、`@` 搜索应用和已启用插件；用上下键选择，Enter/Tab 插入，Escape 关闭，也可从输入区“＋”添加菜单选择。所选引用显示可移除标签；插件与应用独立加载，原生应用目录首次读取可能较慢，超时可单独重试，单侧失败仍保留另一侧结果。模型、思考强度和模式以原生目录为准，点击模型摘要可调整模型、思考强度与模式，包括目录提供的 max 等值；设置失败恢复原生选择，应用目录失败不阻断普通发送。

协作区显示空闲、执行、待处理请求和队列状态，并同步原生 Goal 的目标与状态。点击目标旁的打开按钮或“＋ → 目标”到 Codex 中创建、修改、暂停或恢复目标。审批命令用等宽文本显示，文件变化按文件折叠，展开后查看 diff；超出显示范围时回到 Codex 处理。审批需核对显示内容后确认；命令和文件请求在当前连接支持时可选“允许本次会话”，权限请求仍只用于本回合，问答不预选答案；未完整显示或不支持的请求请回到 Codex 处理。隐藏窗口或切换页面会释放界面订阅，已提交的本机队列仍属于原线程。

验收时在桌面打开同一主线程，连接后分别通过加号、Finder 复制、完整路径文本和截图粘贴添加附件并发送，确认桌面实际可读附件；检查普通文本粘贴、撤销/重做和切换窗口后立即粘贴。加入 $/@ 引用后发送，确认桌面可见执行进度；展开文件审批检查 diff，分别在任一侧完成审批或问答，另一侧待处理项应消失。原生文件引用的显示一致性与真实使用体验仍待用户验收。实现与兼容限制见[桌面协作说明](docs/ARCHITECTURE.md#本机-codex-桌面协作)。

## 阅读与线程大小

Codex、Claude Code、Grok 和 Codex 子智能体上滚到顶部附近时逐页加载较早消息；首次打开定位最新内容，加载和轮询保留阅读位置。父线程没有新消息时，子智能体仍按自身回合刷新状态。搜索命中折叠历史、Claude 过程、工具结果或指令时自动展开目标，保留其他手动折叠选择。搜索、复制、Plan 导出、附件与折叠选择共用稳定消息身份。

每条用户消息按“附件 → 上下文 → 请求正文”排列。AGENTS.md、环境与自动文件说明共用一个“上下文”入口，展开可读原文；纯上下文不产生空气泡或时间线刻度。搜索包含真实正文、问题与回答、附件名及 AGENTS.md 指令，排除自动环境字段。代码、引用和不完整包装保留原文。

左侧时间线使用完整用户消息索引，缩窄刻度与悬停预览；即使较早正文尚未加载或已折叠，也能定位到对应记录，再向前后分页或返回最新内容。Grok 大历史保留最早记录。旧服务使用逐页加载回退。

三种 Provider 按实际字节数统一使用绿、琥珀、红三档颜色，具体阈值见[大小统计规则](docs/ARCHITECTURE.md#大小统计与资源预算)。K／M／G 数值格式和卡片布局保持原有规则；未知或统计中为灰色 `—`，部分结果按已确认下限分色并显示 `≥`。悬停或键盘聚焦可查看准确字节和统计范围。

Codex 统计已验证的明文或 `.jsonl.zst` rollout；Claude 统计主 JSONL 及确认归属的会话目录；Grok 统计专属目录。不计项目工作目录或外部附件，不跟随符号链接。目录在后台有界统计；[资源预算](docs/ARCHITECTURE.md#大小统计与资源预算)说明缓存和刷新边界。

读取缓存合并同源请求，未变化正文复用结果，普通追加只解析新增记录。界面轮询只刷新最新窗口及确有变化的已加载历史；窗口隐藏时暂停界面轮询，后台 monitor 独立运行。

## 会话清理

⌘ 点击进入多选并切换目标，普通点击在多选中切换选中项，否则打开线程；Shift 按当前可见顺序追加区间，连续 Shift 保留锚点。键盘点击按普通选择处理，Ctrl 点击仍打开右键菜单。最多选择 100 项，切换机器、Provider、筛选条件或退出多选会清除选择；轮询只移除消失项。

Codex 普通及归档线程的永久删除预览覆盖完整子线程树、全部 rollout 和合计大小，允许空线程及没有活跃写入者的待回复线程；确认框提示将丢弃问题。存在活动占用、未知引用或压缩历史时拒绝执行；预览后范围变化必须重新预览。原生删除结果须经复检，部分失败或无法核验时明确报告，不能直接重试。归档、恢复和隐藏清理仍采用原有条件。

Claude 改名和删除分别判断资格，兼容性警告不再连带禁用复制和安全管理。原生登记、进程身份与打开文件确定会话占用，已属于其他会话的进程不再锁住同工作目录的全部历史线程。删除预览包含主 transcript 及核实归属的专属目录，不沿续接关系删除其他主会话。菜单提供改名、复制 ID／文件路径／带工作目录的恢复命令、Finder 定位和删除；Finder 仅限本机，恢复命令只复制不执行。

Grok 的活动显示与删除资格分别判断。登记和进程身份确认没有原生占用、记录完整有效时，历史回合缺少结束事件也可进入删除预览。仍被原生程序打开、身份无法确认、记录损坏或尚未写完的线程继续显示具体保护原因。

“更新与维护”会自动分批扫描隐藏线程，显示进度并允许停止；候选与逐项结果分页显示。全部扫描完成后才能确认清理。停止后保留已显示结果，重新扫描建立新快照；切换机器或离开页面会停止后续请求并丢弃迟到响应。删除仍重新核验候选身份、占用和指纹。

## 通知与兼容

Bark 新事件统一使用 `Provider｜状态｜线程标题`，缺失标题时回退项目名；正文最多四个非空行、300 个 Unicode 字符，去掉 Markdown、代码、工具输出和内部信息，不附线程或回合 ID。每个新事件生成一条摘要；回复和授权使用时效提醒，其余使用普通提醒，保留声音和合法显式 URL。

在本机 Probe 设置中确认安装 Claude hooks，接收完成、问题、授权和失败事件；安装 App 不会自动启用。Claude hooks 与同机轮询使用核实的 prompt 身份，同回合每类提醒一次，快速开始新回合不会丢弃有效旧完成或已确认的交互提醒。完成后不单独发送空闲通知。Codex 仍逐问题识别和去重；远端 Claude 继续使用轮询。

Bark 默认关闭，接受 HTTPS 服务和 HTTP 回环服务；危险地址或无效配置会明确失败，不会改用默认服务投递。临时失败采用既有有限重试，次数、间隔和有效期见[通知投递规则](docs/ARCHITECTURE.md#通知与监测)。旧在途消息保留原内容、分段和确认进度；升级不回放历史，不重发成功项。网络结果未知时仍可能重复提醒；“Bark 已受理”只表示服务端确认，设备实收需要单独核对。

API 协议保持 **2**。先升级远程 API，再安装 App；整树删除要求服务声明 `codex_delete_tree` 能力并显式确认后代，旧服务提示升级。旧服务缺少大小或子智能体能力时保留阅读和对应未知／升级提示，缺少历史变更信息时沿用兼容刷新方式。缺少分批扫描能力时使用单次扫描，不显示进度或停止按钮。

Pi、Gotify、网页版和自动 Goal 恢复已退役。迁移仅清理 NexusHub 自有的退休配置与记录，保留 Bark、连接、通知基线及原生 Provider 会话；不会自动删除外部服务。

## 运行方式

### 本机 App

下载 macOS ARM64 DMG，安装后即可读取本机数据。App 更新和远程服务更新分开显示。

当前 macOS 发行包使用 ad-hoc 代码签名，没有 Developer ID 签名或 Apple 公证；updater 的 Minisign 签名用于验证更新归档来源，不能替代系统代码签名。覆盖安装后若 macOS 请求重新授权钥匙串访问，请在系统对话框完成验证，无需重建已保存的远程连接。

### 远程 API

Linux `webd` 只提供管理 API 和健康检查，不提供网页、登录页或静态资源。配置只接受回环监听，远程访问通过 HTTPS 反向代理。打开 App 的“设置 → 远程连接”，填写 HTTPS 地址和管理员 API Key。地址保存在本机设置，Key 保存在 macOS Keychain。

使用导航旁的机器按钮可在“本机”和“远程机器”之间切换。查询缓存、分页、展开状态和写操作都按机器隔离；连接失败不会自动切换目标。

## 快速部署

部署客户端和服务器须先安装 Minisign，并同时取得正式归档、SHA-256 文件与分离签名。首次从旧版本升级时由可信客户端验签，具体步骤见[部署手册](docs/cloud-deploy-runbook.md#安装与升级)。部署参数必须从仓库外提供，示例使用保留域名和占位主机：

```bash
NEXUSHUB_DOMAIN=panel.example.invalid \
  bash scripts/deploy-cloud.sh SSH_HOST ./dist/nexushub-webd-linux-x86_64.tar.gz
```

在服务器生成独立 API Key，并只通过 App 保存：

```bash
sudo /usr/local/bin/nexushub-webd admin key-generate --output /absolute/private/api-key
```

Key 文件保存成功后立即删除。服务器只保存摘要，轮换或撤销会立即使旧 Key 失效。详见[部署手册](docs/cloud-deploy-runbook.md)。

## 隐私与安全

- 发布前扫描源码、可达 Git 历史、构建产物和发布资产，检查私人路径、邮箱、服务器地址、会话内容及凭据；公开依赖的版权署名和自检数据按其许可保留。GitHub 可能保留不可达对象，可达历史扫描不代表平台缓存已全部擦除。
- 远程 API 使用 `x-api-key`，禁止把 Key 放进 URL、日志、浏览器请求或仓库文件。
- 服务保持 systemd 文件隔离；只为已配置的 Provider 会话根开放精确写入路径。
- 文件路径、附件和搜索结果都在当前机器范围内解析，客户端不能提交任意文件路径。

## 发布资产

当前源码版本为 1.3.6；[查看正式发布](https://github.com/lich13/nexushub/releases)。正式发布页包含八项资产：

- macOS ARM64 DMG 与校验文件；
- macOS updater 压缩包与签名；
- 仅包含 `darwin-aarch64` 的 `latest.json`；
- Linux x86_64 `webd` 服务包、校验文件与分离签名。

`darwin-aarch64` 是 updater 使用的平台键，下载文件名中的 `darwin-arm64` 表示同一 macOS ARM64 架构。Linux 服务包不能用于桌面 updater。

## 开发

```bash
corepack pnpm@12.11.2 --dir webui install --frozen-lockfile
node scripts/check-wire-types.mjs
node --test scripts/test-wire-types.mjs
node --test scripts/test-playwright-browser.mjs
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
corepack pnpm@12.11.2 --dir webui typecheck
corepack pnpm@12.11.2 --dir webui test
corepack pnpm@12.11.2 --dir webui build:tauri
bash scripts/test-install-script.sh
python3 scripts/check-versions.py
python3 scripts/check-workspaces.py
python3 scripts/test-install-safety.py
python3 scripts/test-install-signatures.py
python3 scripts/test-privacy-check.py
python3 scripts/test-check-versions.py
python3 scripts/test-check-workspaces.py
python3 scripts/test-tauri-build-config.py
node --test scripts/test-security-audit.mjs
python3 scripts/privacy-check.py --git-objects
git diff --check
```

`rust-toolchain.toml` 固定开发与 CI 工具链；Node、pnpm 和构建工具使用仓库锁定版本。`src-tauri` 是独立 Cargo workspace；最低 Rust 策略、缓存和审计门禁见[构建说明](docs/ARCHITECTURE.md#构建ci-与发布)。本机检查需要 Python 3.11+、ShellCheck 和 Minisign，使用虚构 Provider 数据，不删除本机用户会话。

浏览器回归和 macOS 有头性能检查默认交给现有 GitHub Actions，本机无需安装 Chromium 或 WebKit。CI 先生成当前前端构建，再安装锁文件对应的 Playwright Chromium 与 WebKit；Linux 同时安装系统依赖。上面的浏览器守卫单元测试使用模拟启动器，不安装或启动浏览器。

浏览器验收只使用 Playwright 管理的浏览器，不依赖、不启动本机 Google Chrome，可卸载系统 Chrome。内置 Chromium 的窗口可能显示 Google Chrome for Testing；它与系统 Chrome 是独立安装。全局启动守卫禁止渠道、可执行路径和外部连接覆盖，并在启动探针后核对真实可执行文件位于 Playwright 浏览器缓存，符号链接越界立即失败。

交互预算使用生产前端构建，在 macOS 可见窗口中依次运行两种内置浏览器，测量期间关闭 trace 录制，避免无头窗口调度、其他测试和采集工作干扰绝对耗时。功能回归保留失败 trace；诊断性能问题时可单独加 `--trace=on`，录制结果不能直接作为预算验收。

## 当前文档

- [AGENTS.md](AGENTS.md)：贡献与交付约束。
- [DESIGN.md](DESIGN.md)：界面和交互规范。
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)：模块边界与契约。
- [docs/cloud-deploy-runbook.md](docs/cloud-deploy-runbook.md)：Linux API 部署和升级。
- [docs/progress/MASTER.md](docs/progress/MASTER.md)：维护者状态和已验证范围。

仓库只保留这六份当前 Markdown 文档。后续版本使用普通 Git 增量提交，并在 GitHub Release 提供中文发布说明。
