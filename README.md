# NexusHub 1.2.14

NexusHub 是一个只读优先的会话管理工具，用于在 macOS App 中查看 Codex、Claude Code 和 Grok Build 的本机会话，也可以通过受保护的 API 管理一台远程机器。

## 主要能力

- 按 Provider 搜索会话、搜索当前线程或全部线程，并用时间线定位消息、Plan、附件和工具活动；时间线短线只显示识别到的用户指令。
- 保留文本与工具的原始顺序；连续工具活动可以展开查看，AGENTS.md 和记忆元数据按安全规则处理。
- 复制线程 ID、回复和 Plan；Plan 可保存为 Markdown，文件路径可以复制，macOS 本地路径可以在 Finder 中定位。
- 在 Provider 允许时双击线程标题改名，并对 Codex、Grok 和 Claude Code 提供受保护的归档、删除或恢复操作；Claude Code 兼容常见记录读取，未知或不完整记录保持只读。
- 通过 Bark 接收明确的完成、失败和需要回复通知；Claude 的空结果可回退到当前回合最后一段回复，工具执行中和取消状态不误报完成。

NexusHub 不发送消息、不接管进程、不执行用户命令，也不会安装或配置 Claude Code 或其他 Provider。阅读和展示不会改写原生会话；改名、归档、恢复及删除只在用户确认的范围内执行。

## 1.2.14 通知精简

- 移除 Gotify 设置、测试入口和发送能力，通知继续使用 Bark。
- 升级自动清理旧 Gotify 配置、加密 Token、待投递记录和测试任务；Bark 设置、成功记录、去重状态与启用基线保留，不重复补发历史。
- 保留 AGENTS.md 完整折叠修复：`</INSTRUCTIONS><environment_context>` 同行时，内部各级标题都留在折叠区，普通请求保持原顺序。

先升级远程 API，再安装本机 App。API 协议仍为 **2**，现有连接与用户会话继续使用。曾单独部署的 Gotify 服务需由管理员移除；NexusHub 不自动删除外部服务。

## 1.2.12 界面精简

移除子智能体汇总中的通用完整性说明，以及归档清理、隐藏线程清理的重复说明。状态数量、未知或不可用条目、清理预览和逐项执行结果继续保留。

## 1.2.11 子智能体活动与统计

- Codex 原生“开始工作”“已完成”“已中断”和交互记录按发生顺序显示；子智能体结束后，开始记录仍保留。
- 活动行、列表和详情统一使用创建任务名；缺少任务名时再使用已验证任务路径、线程标题或昵称。
- 线程详情增加直属子智能体汇总。点击可查看运行中、已完成及异常状态的子智能体；进入某个子智能体后，只统计它自己的直属下级。主线程列表不增加数量。
- 父线程没有新消息时仍刷新子智能体状态；未确认记录显示未知或不可用，避免误报完成。
- 默认隐藏完整的内部页面标记 `external_codex_apps_open_page`，显示、复制、Plan 导出和搜索同步清理；代码、引用示例及无法确认的结构保留。

远程服务升级到 1.2.11 后可显示直属汇总，API 协议仍为 **2**。旧服务仍可阅读原有详情，并提示升级后查看汇总。

## 1.2.10 修复

- 修复切换远程机器后会话列表和设置可能一直加载的问题：钥匙串访问按顺序在后台执行，避免阻塞界面所用的异步运行时。
- 钥匙串读取超过 15 秒会显示就地错误；原生操作仍在执行时，后续调用继续受串行保护。
- 保存与移除连接保持完整事务，机器切换后重新核对目标，丢弃旧连接结果。现有远程连接无需重建。

## 1.2.9 阅读体验

- Codex、Claude Code 和 Grok 使用更紧凑的阅读布局，正文与活动摘要对齐，减少重复标签和空白。
- 桌面回复复制按钮在悬停或键盘聚焦时出现，触屏保持可见；操作不改变正文高度。
- Codex 委派活动显示子智能体名称与状态，点击可在右侧查看委派内容、消息、Plan、工具及附件。宽窗口并排显示，窄窗口覆盖显示；支持嵌套返回、Escape 关闭和焦点恢复。
- 子智能体关联由原生身份和父子关系验证，运行状态来自子线程自身回合。缺失或有歧义的记录显示原因，不通过相似名称猜测。
- 修复 Hook 集成测试对编译期二进制路径的硬依赖，继续使用本次 Cargo 构建的真实 CLI 验证通知行为。

远程 API 协议保持 **2**。使用远程子智能体详情前，请先升级服务至 1.2.9；旧服务缺少该能力时会提示升级。现有搜索、线程管理、通知、Keychain 连接与机器隔离继续保留。

## 运行方式

### 本机 App

下载 macOS ARM64 DMG，安装后即可读取本机数据。App 更新和远程服务更新分开显示。

当前 macOS 发行包没有 Developer ID 签名或 Apple 公证；updater 的 Minisign 签名用于验证更新归档来源，不能替代系统代码签名。覆盖安装后若 macOS 请求重新授权钥匙串访问，请在系统对话框完成验证，无需重建已保存的远程连接。

### 远程 API

Linux `webd` 只提供管理 API 和健康检查，不提供网页、登录页或静态资源。打开 App 的“设置 → 远程连接”，填写 HTTPS 地址和管理员 API Key。地址保存在本机设置，Key 保存在 macOS Keychain。

使用导航旁的机器按钮可在“本机”和“腾讯云”之间切换。查询缓存、分页、展开状态和写操作都按机器隔离；连接失败不会自动切换目标。

## 快速部署

部署参数必须从仓库外提供，示例使用保留域名和占位主机：

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

## 1.2.14 发布资产

[查看 v1.2.14 发布说明](https://github.com/lich13/nexushub/releases/tag/v1.2.14)。发布页包含七项资产：

- macOS ARM64 DMG 与校验文件；
- macOS updater 压缩包与签名；
- 仅包含 `darwin-aarch64` 的 `latest.json`；
- Linux x86_64 `webd` 服务包与校验文件。

`darwin-aarch64` 是 updater 使用的平台键，下载文件名中的 `darwin-arm64` 表示同一 macOS ARM64 架构。Linux 服务包不能用于桌面 updater。

## 开发

```bash
corepack pnpm@11.0.8 --dir webui install --frozen-lockfile
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
corepack pnpm@11.0.8 --dir webui typecheck
corepack pnpm@11.0.8 --dir webui test
corepack pnpm@11.0.8 --dir webui build:tauri
corepack pnpm@11.0.8 --dir webui test:browser
bash scripts/test-install-script.sh
python3 scripts/privacy-check.py --git-objects
git diff --check
```

`src-tauri` 是独立 Cargo workspace。测试使用虚构 Provider 数据，不删除本机用户会话。

## 当前文档

- [AGENTS.md](AGENTS.md)：贡献与交付约束。
- [DESIGN.md](DESIGN.md)：界面和交互规范。
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)：模块边界与契约。
- [docs/cloud-deploy-runbook.md](docs/cloud-deploy-runbook.md)：Linux API 部署和升级。
- [docs/progress/MASTER.md](docs/progress/MASTER.md)：维护者状态和已验证范围。

仓库只保留这六份当前 Markdown 文档。后续版本使用普通 Git 增量提交，并在 GitHub Release 提供中文发布说明。
