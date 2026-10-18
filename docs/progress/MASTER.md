# NexusHub 当前状态（维护者）

## 当前版本

当前维护源码版本为 1.3.4，基线为 `cfb1be1 / v1.3.3`。本轮修复发送兼容提示、文件身份误判与剪贴板附件，并修复候选中系统截图粘贴的通知通道提前关闭问题。用户已确认修复候选的截图及其余约定验收项目通过；正式 `v1.3.4` 已从 `4d752c6` 发布八项资产。历史版本的构建、候选和实机结果由 Git 与 [Release](https://github.com/lich13/nexushub/releases) 保留，不能代替本轮证据。完整回归以对应提交的 [CI](https://github.com/lich13/nexushub/actions/workflows/ci.yml) 为准。

## 核实与实现

| 工作块 | 核实结果与实现 | 提交 |
|---|---|---|
| 发送兼容 | 顶部警告仅比较 build 号；20586 与 20770 的现用方法表一致。已移除常驻提示，保留按实际协议、状态及 owner 核验，增加原生 request-timeout 分类。未知写入版本仍拒绝，不自动试探或重发。 | `aab2431` |
| 文件身份 | 隔离样本确认扩展属性更新可只改变 ctime，原检查会误拒。新选择的图片与小文件记录有界 SHA-256 和权限身份，只有单纯 ctime 变化且内容与其他身份一致才继续。改写、替换、权限及符号链接变化仍拒绝；旧引用保持严格检查。错误细分，失败保留草稿，重新选择或移除附件清除对应操作提示。 | `f234a1c` |
| 剪贴板 | 原实现缺少 Finder 文件、完整路径文本和 JPEG 入口。现由 Rust 读取原生剪贴板并整批核验/去重，Finder 文件优先于缩略图；普通正文或无效路径回退文本。合并绑定请求、处理焦点恢复、拒绝迟到结果与重复导入。协议 2 的现有附件响应增加可选 pasteDisposition，不接收客户端路径或图片字节。 | `2ab29df` |
| 附件通知 | 候选实机发现 bounds 操作的临时 Tauri Channel 会提前注销前端回调。仅 bind 建立通道，其余动作复用已绑定接收区，保留原有 wire 字符串与身份检查。 | `d37d9ef` |

截图中那一次“引用变化”的具体原因没有充分证据，不能把隔离 ctime 复现写成它的确定根因。未来破坏性原生协议变化仍可能需要适配，不能承诺任意未知协议自动可用；build 字符串本身不限制兼容协议下的发送。

实现参数、旧字段兼容与安全边界见[桌面协作架构](../ARCHITECTURE.md#本机-codex-桌面协作)。没有批量升级依赖；AppKit 仅启用已锁定依赖的 NSPasteboardItem 特性。原生会话、旧加密队列、Keychain 和私有附件生命周期保持。

## 本机必要检查

以下结果对应 `2ab29df` 的产品与测试源码，均在核心阶段实际执行。首次失败与修正单列；收尾的版本/文档和候选构建结果另行记录，不把局部复验写成全量回归。入口与完整 CI 命令见 [README 开发](../../README.md#开发)。

| 范围 | 实际命令与最终结果 |
|---|---|
| Rust core | `cargo test -p nexushub-core --locked codex::desktop`：131 项通过；同命令过滤 `db::desktop_submissions`：15 项通过；`--test wire_contract --test contract_registry`：各 6 项通过，均退出 0。覆盖内容/权限/替换/旧密文、原生超时不重试及现有桌面状态语义。 |
| Tauri | `cargo test --manifest-path src-tauri/Cargo.toml --locked codex_desktop`：51 项通过、退出 0；新增 10 项专用 pasteboard 测试，覆盖 Finder 多文件、路径文本、普通正文、去重、PNG/TIFF/JPEG、目录/远程地址拒绝和容量边界。测试未读写真实剪贴板。 |
| 格式与 Clippy | 两套 workspace 的 `cargo fmt ... --all -- --check`，core 与 Tauri 的 `cargo clippy ... --all-targets --locked -- -D warnings` 均退出 0。 |
| WebUI | `corepack pnpm@12.11.2 --dir webui test --run` 指定 useDesktopAttachments、DesktopReferenceInteractions、useCodexDesktop、useCodexDesktopLifecycle、api/desktop、desktopReferences 六文件，99 项通过；typecheck 和 build:tauri 均退出 0，构建转换 1,963 个模块。覆盖无意义 build 提示、绑定等待、未绑定文本回退、重复/迟到事件、错误清除及草稿保留。 |
| 契约与配置 | `node scripts/check-wire-types.mjs` 与 `node --test scripts/test-wire-types.mjs` 退出 0，后者 8 项通过；`python3 scripts/check-versions.py`、`python3 scripts/check-workspaces.py` 退出 0。此时自身包版本仍为 1.3.3。 |
| 隐私与差异 | 三个提交之后 `python3 scripts/privacy-check.py --git-objects` 检查 3,151 项、零发现、退出 0；工作区及暂存 `git diff --check` 均退出 0。 |

初次 fmt 检查因固定错误文案排版退出 1，修正后两套检查通过；core 桌面测试最初为 130 通过、1 失败，原因是既有身份字段集合未纳入可选 content_proof，更新预期后全部 131 项通过。新增原生测试最初修正了临时目录 canonical URL 的预期和超像素预算 PNG 样本，最终 51 项 Tauri 测试通过。前端补充未绑定路径粘贴回归后，最终六文件 99 项通过。没有放宽产品身份、权限或容量保护来消除测试失败。

本机未重复执行全量 Rust/WebUI、浏览器或 headed 性能；这些仍由既有三项 CI 执行。未执行真实回合、真实剪贴板、两端附件显示或用户视觉验收。

## 收尾与候选

1.3.4 版本同步只更新现有 manifest 和两份锁文件中的自身包条目，不改变依赖解析。六份当前文档及既有发布说明生成器已同步，说明保留中文六节和八项正式资产。

收尾检查：`check-versions.py`、`check-workspaces.py`、两套 workspace 的 `cargo metadata --locked --no-deps`、`check-wire-types.mjs` 均退出 0；版本、workspace、Tauri 构建配置与 wire 工具测试分别为 8、7、2、8 项通过。`test-install-script.sh` 初次因 1.3.4 尚未登记中文六节说明而退出 1，补齐发布元数据后复验退出 0，其中安装安全 13 项、Minisign 验证 9 项通过。随后隐私扫描检查 3,151 项、零发现，`git diff --check` 退出 0。未因版本同步重复核心阶段已通过的功能测试，也未把这些检查记作完整回归。

候选由 `0466d15` 构建，沿用原 bundle 身份、生产参数及增量缓存；保持 updater 配置，但不生成或发布正式签名归档。helper 的 `cargo build --release --package nexushub-webd --locked` 与 Tauri 的 app-only `build --bundles app -- --locked` 均退出 0，复用核心阶段的有效 WebUI 生产产物。App 版本 1.3.4、标识 `com.lich13.nexushub`、架构 arm64；`codesign --verify --deep --strict` 退出 0。这是本机 ad-hoc 候选签名，不能代替正式更新归档的 Minisign 验证。

候选主程序 SHA-256 为 `f94055ada004a1e673fe42b0b1107b12bcda80bf5a1c2b434cabffd3bb07419a`，内置 helper 为 `898ee4f7b12a8a2e55187cb2632c6bbc1ec48abbbb3be06219fe7177350f1cc7`；构建包隐私检查 529 项、零发现、退出 0。同卷原子交换已完成，安装后的版本、架构、主程序/helper 哈希及签名完整性与候选一致。

启动命令退出 0，首次进程检查未在检查窗口内捕获 App 而退出 1；未再次启动或回滚。随后只读核对已运行 App 的可执行文件身份、版本及连续存活，退出 0，确认候选已启动。没有把启动检查当作真实剪贴板、发送或视觉验收。

候选安装时唯一 App 回退材料为 `dist/candidates/1.3.4/rollback/NexusHub.app`，保留安装前正式 1.3.3，其主程序 SHA-256 为 `4fb2fb4b50ff27c97d7c70f902518524d92e1d62c14fb5492464e7b20e5ceac7`，签名完整性通过。旧候选回退和交换暂存已精确清理，移除文件的逻辑大小为 79,798,810 字节、分配大小为 79,822,848 字节；未把这两个数值当作磁盘净空闲增量。正式发布资产、依赖与构建缓存、业务数据均保留。资源占位已恢复，临时构建配置和目录已清理。后续修复候选、用户验收及正式安装结果见下文。

## 候选与用户验收

首轮候选的纯图片粘贴验收失败。只读元数据确认系统剪贴板提供有效单帧 PNG/TIFF、处于现有大小和解码预算内；已安装候选哈希与构建相符。独立 WKWebView 的一次有界复现收到 `Files` / `image/png` paste 事件，未读取图片正文、改写真实剪贴板或发送回合。现有 15 项原生剪贴板测试复验退出 0；这些结果不能替代 NexusHub 内的附件显示验收。

锁定的 Tauri 实现每次解析 Channel 参数都会创建独立生命周期；原附件入口把同一个回调反复传为 Channel，首次 bounds 完成时的临时实例就发送结束帧并注销前端回调。修复改为只解析 JavaScriptChannelId，仅 bind 创建通道，其他操作使用已有接收区。线上 onEvent 字符串、DTO、协议 2、身份及容量校验均不变；已知原生参数类型变化同步到 wire 结构清单。

此次修复的本机检查：`cargo test --manifest-path src-tauri/Cargo.toml --locked --test attachment_channel` 的入口生命周期守卫 1 项、同命令过滤 `codex_desktop::attachments` 的边界测试 4 项、WebUI 的 `api/desktop.test.ts` 与 `useDesktopAttachments.test.ts` 共 36 项均通过、退出 0。新增用例覆盖连续 bounds 后粘贴事件仍可达、关闭/切换后的迟到事件隔离，并以旧结束帧和旧原生签名作负面对照；没有把 mock 或源码守卫记成真实 App 发送。WebUI typecheck、两套 fmt、Tauri `clippy --all-targets --locked -- -D warnings`、Rust wire 单项与 TypeScript wire 检查均退出 0。新增测试初次 fmt 退出 1，格式修正后通过；一轮隐私扫描未取得退出码，补跑确认检查 3,169 项、零发现、退出 0。

修复前 `ee57644` 的 CI 39104232687 首次 Frontend/Backend 通过，macOS WebKit 性能切换 p95 为 116ms，超过 100ms；同提交仅重跑 macOS 后三项均通过，WebKit 83ms、Chromium 53ms。该结果不代表本次原生修复已完成全量 CI，新提交仍使用既有 workflow。

修复提交 `d37d9ef` 已推送 `main`。本次三项 CI run `39110412690` 全部成功：Backend `117625157756`、macOS Tauri `117625157842`、Frontend `117625157967`。候选由该提交的 Tauri 原生 App 构建，主程序 SHA-256 为 `8d0310ee01acb7715341c3de0912a7d45b7466ae54cd1433928ae8b277b26778`，helper 沿用 `898ee4f7b12a8a2e55187cb2632c6bbc1ec48abbbb3be06219fe7177350f1cc7`；1.3.4、`com.lich13.nexushub`、arm64、深度签名检查和 payload 隐私检查（529 项、零发现）均通过。旧候选已精确替换并启动，PID 与安装路径身份匹配且连续存活；助手没有操作真实剪贴板或发送回合。2026-10-18 用户确认系统截图粘贴恢复，随后明确确认 Finder 文件、路径文本、加号选择器、普通文本/焦点与附件发送等其余约定项目也已通过。

前提：在 Codex 原生桌面打开同一本机主线程，再在候选 NexusHub 中连接。远程、归档和子线程继续只读。

| 项目 | 操作与预期 | 本轮结果 |
|---|---|---|
| 兼容提示与发送 | 连接后不再出现 build 常驻警告；输入普通消息发送，原生桌面显示执行。 | 用户确认候选通过 |
| 原生选择器 | 用“＋”添加图片和普通文件后发送，原生桌面实际可读附件。 | 用户确认候选通过 |
| Finder 文件 | 在 Finder 复制单个/多个文件并粘贴；得到文件标签，预览缩略图不替代原文件。 | 用户确认候选通过 |
| 完整路径 | 粘贴每行一个绝对路径、~/ 路径或本机 file URL，含中文、空格或成对引号；整批有效才附加，无效路径保留文字。 | 用户确认候选通过；具体格式边界由自动测试覆盖 |
| 图片 | 粘贴截图以及 PNG/TIFF/JPEG 来源；发送后确认图片实际可用，重复事件不产生重复附件。 | 修复后系统截图粘贴与附件发送由用户确认通过；未逐一声明其他图片来源的实机结果 |
| 文本与焦点 | 普通多行正文保持换行、光标、撤销/重做；切换窗口或关闭文件选择器后立即粘贴可用。 | 用户确认候选通过 |
| 错误与草稿 | 文件真正变化时拒绝发送并保留草稿；重新选择或移除附件后相应错误消失，队列结果不明项仍保留。 | 隔离自动测试通过；额外真实文件变化未单独验收 |

## 正式发布

tag `v1.3.4` 指向 `4d752c61658ac2ffc01bca9c4e8463a37f33edd6`；该提交仅在修复候选 `d37d9ef` 上增加验收记录。对应 [CI 39111842540](https://github.com/lich13/nexushub/actions/runs/39111842540) 的 Frontend、Backend、macOS Tauri 三项全部成功。[Release 39113978616](https://github.com/lich13/nexushub/actions/runs/39113978616) 成功生成并发布八项资产，保留中文六节说明；正式包直接复用该次 CI 产物。

DMG 与 Linux 包的发布 SHA-256 核对通过；Linux 及 macOS updater 包使用仓库固定公钥验签并核对已签版本 1.3.4，均退出 0。`latest.json` 的顶层版本、`darwin-aarch64` 平台和归档映射一致。初次人工核验把 Tauri base64 封装签名直接传给 Minisign 而失败，改用现有 `install-safety.py verify` 读取该格式后通过，未修改签名或信任根。

远程 API 通过现有 `scripts/deploy-cloud.sh` 唯一执行升级，退出 0；此前的本机参数解析失败未进入安装。正式 Linux 归档 SHA-256 为 `12a7983cf5ce55c55ce7f274156fb083af386b25b5168b844c26b45a147b82b1`，实际运行进程和安装程序的 SHA-256 均为 `040cff10d6e166b2d13098b843e6cc4fa3b09e7bedf8001145e677e07ae1c914`，与归档二进制一致。运行版本 1.3.4、服务 active、回环与既有 HTTPS 健康接口均 200、未认证 API 为 401；`ProtectSystem=full`、`ProtectHome=read-only`、`NoNewPrivileges=yes`、`PrivateTmp=yes` 均核对一致。配置只按安装器既有逻辑处理；本轮未保存部署前配置哈希，不能宣称已完成前后逐字节比较。未执行远程业务操作，部署、载荷与事务暂存已清理。

远程升级成功后，macOS 正式 App 从已验签 updater 包安装，使用同卷 `renameatx_np(RENAME_SWAP)` 原子交换，退出 0。安装版本 1.3.4、标识 `com.lich13.nexushub`、arm64 均一致，主程序 SHA-256 为 `4df57050eb905dc63483d8f5bcdb3aad11ba7296a92a71aeb1b6c4c60fd686fc`，helper 为 `c604d33f56b12c248eceae712dd0c396158adf51f3caf7762d6cebd398fdccb8`。深度签名、payload 隐私扫描（529 项、零发现）、启动与进程身份检查均通过；正式 App 的功能与视觉未由助手重复验收。

八项正式资产保存在 `dist/releases/1.3.4/`，唯一 App 回退材料为已验收的 `dist/candidates/1.3.4/NexusHub.app`，主程序哈希仍为 `8d0310ee01acb7715341c3de0912a7d45b7466ae54cd1433928ae8b277b26778`。旧 1.3.3 回退、重复解包/交换副本及本轮下载暂存已逐一核对后清理；清理目标逻辑大小合计 115,607,690 字节、分配大小 116,649,984 字节，不作为净磁盘空间增量。正式资产、共享缓存、配置、凭据、原生会话及私有附件保留。

## 保留边界与交付条件

桌面始终是执行 owner，不启用独立 CLI、PTY、云 API 或 remoteControl，不改写原生会话和队列。API 协议保持 2，远程 allowlist、tombstone、Keychain、安全检查及“结果不明不重发”保持；Goal 直接管理、其他 Provider 与未知原生协议均不扩展。

本轮已按候选验收、同提交 CI、八项正式资产核验、远程 API 升级、正式 App 安装的顺序交付。助手仅执行必要部署、版本和启动检查，用户确认的是修复候选的约定流程；正式安装后的日常功能与视觉仍由用户验收。保留一套已验收候选回退，不回滚业务数据；清理仅限本轮明确归属的暂存。
