# NexusHub

NexusHub 是一个会话阅读与管理工具，用于在 macOS App 中查看 Codex、Claude Code 和 Grok Build 的本机会话，也可以通过受保护的 API 管理一台远程机器。本机 Codex 主线程支持连接原生桌面继续协作。

## 主要能力

- 按 Provider 搜索会话、搜索当前线程或全部线程，并用时间线定位消息、Plan、附件和工具活动；时间线短线只显示识别到的用户指令。
- 保留文本与工具的原始顺序；连续工具活动可以展开查看。自动上下文合并为一行默认收起，真实请求与附件正常显示。
- 复制线程 ID、回复和 Plan；Plan 可保存为 Markdown，文件路径可以复制，macOS 本地路径可以在 Finder 中定位。
- 在 Provider 允许时双击线程标题改名，并对 Codex、Grok 和 Claude Code 提供受保护的归档、删除或恢复操作；Claude Code 兼容常见记录读取，未知或不完整记录保持只读。
- 通过 Bark 接收明确的完成、失败和需要回复通知；Claude 的空结果可回退到当前回合最后一段回复，工具执行中和取消状态不误报完成。
- 在本机 Codex 新建或继续主线程，发送消息、排队、补充与停止；选择原生模型、思考强度、默认或 Plan 模式，添加文件、图片、技能、应用和插件引用，并处理受支持的审批与问答。

对话和工具始终由原生 Codex 桌面执行；NexusHub 不独立运行模型或安装 Provider。远程、归档及子线程，以及 Claude Code、Grok 的对话仍只读。阅读和展示不会改写原生会话；改名、归档、恢复及删除只在用户确认的范围内执行。

## 本机 Codex 协作

打开本机 Codex 主线程，在协作区选择“在 Codex 中打开”，原生桌面打开对应线程后点击“连接”。新建时先选择工作目录，空线程会在原生桌面打开；尚未发送消息时没有对话内容。需要已安装并运行兼容的 Codex 桌面；无法确认线程归属或协议时，会保留阅读并说明连接失败。

输入后按发送按钮或 ⌘/Ctrl+Enter。运行时“排队”留作下一回合，“补充”发给当前回合，“停止”中断当前回合并暂停本线程待发送队列。待发送内容在本机加密保存，断线或重启后由你确认继续；结果不明的记录需先在 Codex 核对，不会自动重发。队列与原生桌面自己的排队内容分别保留。

文件和图片由原生选择器选择，技能、应用及已启用插件来自原生可用目录。审批需核对显示内容后确认，问答不预选答案；未完整显示或不支持的请求请回到 Codex 处理。隐藏窗口或切换页面会释放界面订阅，已提交的本机队列仍属于原线程。实现与兼容限制见[桌面协作说明](docs/ARCHITECTURE.md#本机-codex-桌面协作)。

## 阅读与线程大小

Codex、Claude Code、Grok 和 Codex 子智能体上滚到顶部附近时逐页加载较早消息；首次打开定位最新内容，加载和轮询保留阅读位置。父线程没有新消息时，子智能体仍按自身回合刷新状态。搜索命中折叠历史、工具结果或指令时自动展开目标，保留其他手动折叠选择。搜索、复制、Plan 导出、附件与折叠选择共用稳定消息身份。

每条用户消息按“附件 → 上下文 → 请求正文”排列。AGENTS.md、环境与自动文件说明共用一个“上下文”入口，展开可读原文；纯上下文不产生空气泡或时间线刻度。搜索包含真实正文、问题与回答、附件名及 AGENTS.md 指令，排除自动环境字段。代码、引用和不完整包装保留原文。

左侧时间线使用完整用户消息索引，缩窄刻度与悬停预览；即使较早正文尚未加载或已折叠，也能定位到对应记录，再向前后分页或返回最新内容。Grok 大历史保留最早记录。旧服务使用逐页加载回退。

三种 Provider 按实际字节数统一使用绿、琥珀、红三档颜色，具体阈值见[大小统计规则](docs/ARCHITECTURE.md#大小统计与资源预算)。K／M／G 数值格式和卡片布局保持原有规则；未知或统计中为灰色 `—`，部分结果按已确认下限分色并显示 `≥`。悬停或键盘聚焦可查看准确字节和统计范围。

Codex 统计已验证的 rollout；Claude 统计主 JSONL 及确认归属的会话目录；Grok 统计专属目录。不计项目工作目录或外部附件，不跟随符号链接。目录在后台有界统计；[资源预算](docs/ARCHITECTURE.md#大小统计与资源预算)说明缓存和刷新边界。

读取缓存合并同源请求，未变化正文复用结果，普通追加只解析新增记录。界面轮询只刷新最新窗口及确有变化的已加载历史；窗口隐藏时暂停界面轮询，后台 monitor 独立运行。

## 会话清理

Grok 的活动显示与删除资格分别判断。登记和进程身份确认没有原生占用、记录完整有效时，历史回合缺少结束事件也可进入删除预览。仍被原生程序打开、身份无法确认、记录损坏或尚未写完的线程继续显示具体保护原因。

“更新与维护”会自动分批扫描隐藏线程，显示进度并允许停止；候选与逐项结果分页显示。全部扫描完成后才能确认清理。停止后保留已显示结果，重新扫描建立新快照；切换机器或离开页面会停止后续请求并丢弃迟到响应。删除仍重新核验候选身份、占用和指纹。

## 通知与兼容

Bark 接受 HTTPS 服务和 HTTP 回环服务；危险地址或无效配置会明确失败，不会改用默认服务投递。Bark 对临时失败进行有限重试，并尊重服务端的重试等待时间；次数、间隔和有效期见[通知投递规则](docs/ARCHITECTURE.md#通知与监测)。已成功分段不重发，重试会重新核对身份、问题状态、开关和目标。网络结果未知时仍可能重复提醒；“Bark 已受理”只表示服务端确认，设备实收需要单独核对。

API 协议保持 **2**。先升级远程 API，再安装 App；旧服务缺少大小或子智能体能力时保留阅读和对应未知／升级提示，缺少历史变更信息时沿用兼容刷新方式。缺少分批扫描能力时使用单次扫描，不显示进度或停止按钮。

Pi、Gotify、网页版和自动 Goal 恢复已退役。迁移仅清理 NexusHub 自有的退休配置与记录，保留 Bark、连接、通知基线及原生 Provider 会话；不会自动删除外部服务。

## 运行方式

### 本机 App

下载 macOS ARM64 DMG，安装后即可读取本机数据。App 更新和远程服务更新分开显示。

当前 macOS 发行包没有 Developer ID 签名或 Apple 公证；updater 的 Minisign 签名用于验证更新归档来源，不能替代系统代码签名。覆盖安装后若 macOS 请求重新授权钥匙串访问，请在系统对话框完成验证，无需重建已保存的远程连接。

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

[查看 v1.3.0 发布说明](https://github.com/lich13/nexushub/releases/tag/v1.3.0)。发布页包含八项资产：

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
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
corepack pnpm@12.11.2 --dir webui typecheck
corepack pnpm@12.11.2 --dir webui test
corepack pnpm@12.11.2 --dir webui build:tauri
NEXUSHUB_BROWSER_PREVIEW=1 corepack pnpm@12.11.2 --dir webui exec playwright test --grep-invert @desktop-performance
NEXUSHUB_BROWSER_PREVIEW=1 corepack pnpm@12.11.2 --dir webui exec playwright test --grep @desktop-performance --workers=1 --trace=off --headed
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

`rust-toolchain.toml` 固定开发与 CI 工具链；Node、pnpm 和构建工具使用仓库锁定版本。`src-tauri` 是独立 Cargo workspace；最低 Rust 策略、缓存和审计门禁见[构建说明](docs/ARCHITECTURE.md#构建ci-与发布)。测试需要 Python 3.11+、ShellCheck、Minisign 及锁文件对应的 Playwright 浏览器，使用虚构 Provider 数据，不删除本机用户会话。

交互预算使用生产前端构建，在 macOS 可见窗口中依次运行两种浏览器，测量期间关闭 trace 录制，避免无头窗口调度、其他测试和采集工作干扰绝对耗时。功能回归保留失败 trace；诊断性能问题时可单独加 `--trace=on`，录制结果不能直接作为预算验收。

## 当前文档

- [AGENTS.md](AGENTS.md)：贡献与交付约束。
- [DESIGN.md](DESIGN.md)：界面和交互规范。
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)：模块边界与契约。
- [docs/cloud-deploy-runbook.md](docs/cloud-deploy-runbook.md)：Linux API 部署和升级。
- [docs/progress/MASTER.md](docs/progress/MASTER.md)：维护者状态和已验证范围。

仓库只保留这六份当前 Markdown 文档。后续版本使用普通 Git 增量提交，并在 GitHub Release 提供中文发布说明。
