# NexusHub 1.2.7

NexusHub 是一个只读优先的会话管理工具，用于在 macOS App 中查看 Codex、Claude Code、Grok Build 和 Pi 的本机会话，也可以通过受保护的 API 管理一台远程机器。

## 主要能力

- 按 Provider 搜索会话、搜索当前线程或全部线程，并用时间线定位消息、Plan、附件和工具活动；时间线短线只显示识别到的用户指令。
- 保留文本与工具的原始顺序；连续工具活动可以展开查看，AGENTS.md 和记忆元数据按安全规则处理。
- 复制线程 ID、回复和 Plan；Plan 可保存为 Markdown，文件路径可以复制，macOS 本地路径可以在 Finder 中定位。
- 在 Provider 允许时双击线程标题改名，并对 Codex、Grok、Pi 和 Claude Code 提供受保护的归档、删除或恢复操作；Claude Code 兼容常见记录读取，未知或不完整记录保持只读。
- 通过 Bark 接收明确的完成、失败和需要回复通知；Claude 的空结果可回退到当前回合最后一段回复，工具执行中和取消状态不误报完成。

NexusHub 不发送消息、不接管进程、不执行用户命令，也不会安装或配置 Claude Code、Pi 或其他 Provider。原生会话文件保持不变。

## 运行方式

### 本机 App

下载 macOS ARM64 DMG，安装后即可读取本机数据。App 更新和远程服务更新分开显示。

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

- 仓库、测试夹具、构建产物和发布包不包含用户路径、邮箱、服务器地址、会话内容、凭据或 Key。
- 远程 API 使用 `x-api-key`，禁止把 Key 放进 URL、日志、浏览器请求或仓库文件。
- 服务保持 systemd 文件隔离；只为已配置的 Provider 会话根开放精确写入路径。
- 文件路径、附件和搜索结果都在当前机器范围内解析，客户端不能提交任意文件路径。

## 1.2.7 发布资产

[查看 v1.2.7 发布说明](https://github.com/lich13/nexushub/releases/tag/v1.2.7)。发布页包含七项资产：

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
