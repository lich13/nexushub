# NexusHub 当前状态（维护者）

## 当前版本

当前维护版本为 1.3.1。正式发布状态、中文说明与八项资产以 [GitHub Release](https://github.com/lich13/nexushub/releases) 为准，对应提交的完整回归结果见 [CI](https://github.com/lich13/nexushub/actions/workflows/ci.yml)。历史改动与测量记录由 Git 和 Release 保留。

## 当前改动

- T0：只读核对桌面 build 20586 源码，保留带源文件哈希的脱敏合成 fixture；没有把源码派生样例称为原生 snapshot。
- T1：`$` / `@` 内联补全、键盘和组合输入处理、占位文字与标签同步；WebUI 仅提交 handle 和 UTF-8 区间，Rust 校验后生成 `text_elements[].byteRange`，保留结构化引用和旧加密队列正文逐字节兼容。
- T3/T6：仅完整命令/文件请求声明“允许本次会话”，旧 `allow` 保持兼容，权限仍为 turn scope。固定方法和版本集中管理，连接/owner/协议/失步/超时使用可区分的安全中文文案。
- T4：目录缓存锁不跨 I/O，同 key 并发读取合并；保持现有容量和有效期，失败不缓存。引用和模型目录支持重试，部分来源失败不丢弃其他结果；设置验证、取消、切换身份与失效 cursor 都保持核验。插件 canonical name 与原生 mention 对齐，友好标题保留在说明中。
- T5：原生事件触发当前线程详情刷新，慢读取不被高频事件取消，之后补尾刷新；隐藏/切换/detach 丢弃旧订阅和刷新，不取消已授权后台队列。
- T2 与 T3 未核实部分：普通文件仍使用原兼容前缀，图片维持原生附件和身份复核；不加入未证实的文件 cancel、永久授权或策略修订。API 协议 2、桌面 owner、远程只读和不独立执行 turn 的边界保持。

## 验证范围

本轮基于 `7573b86`，本机必要检查针对实际工作树执行，未运行原生 App、浏览器功能或视觉验收。已执行结果如下；完整回归、跨平台构建及 Chromium/WebKit、macOS headed 性能门禁仍交对应提交的现有三项 CI。命令入口见 [README 开发](../../README.md#开发)。

| 范围 | 本轮命令与结果 |
|---|---|
| Rust core | `cargo test -p nexushub-core --locked`、`cargo clippy -p nexushub-core --all-targets --locked -- -D warnings` 最终退出 0；475 个单元测试及执行的集成测试通过，既有 ignored 性能/大规模项未冒充运行。 |
| Tauri | `cargo test --manifest-path src-tauri/Cargo.toml --locked` 102 项通过；随后只调整目录准备前的身份捕获时机，对 `codex_desktop::` 的 22 项测试及全目标 clippy 复验退出 0。 |
| 前端 | `corepack pnpm@12.11.2 --dir webui typecheck`、`test`、`build:tauri` 最终退出 0，51 个文件、681 项测试通过；生产构建转换 1,959 个模块。 |
| 契约与配置 | `node scripts/check-wire-types.mjs`、`node --test scripts/test-wire-types.mjs`、`python3 scripts/check-versions.py`、`check-workspaces.py`、`test-tauri-build-config.py` 退出 0。版本检查此时核对的是 1.3.0 manifest。 |
| 安装、格式与隐私 | `bash scripts/test-install-script.sh`、两套 workspace 的 fmt check、`python3 scripts/privacy-check.py --git-objects` 与 `git diff --check` 退出 0；隐私扫描覆盖本轮源码提交与当前文档工作树。签名/安装行为未改。 |

首次 core 编译因私有请求方法收紧后的旧测试调用和新增测试比较类型失败（101），修正后重跑完整 core 命令通过。首次前端全量测试发现 DTO 名单遗漏（1），补齐后重跑统一 typecheck 和全量测试通过。早期目录分页测试使用超出服务端页上限的 fixture 并错误期待累计页，已按实际有界分页契约修正；单文件 tsc 尝试因 TS5112 的命令用法失败，后续统一 typecheck 覆盖。

收尾仅同步 1.3.1 版本、当前文档和六节发布说明，没有继续修改产品代码或依赖。两套 workspace 的 locked metadata、`check-versions.py --tag v1.3.1`、`test-tauri-build-config.py`、`test-install-script.sh`（含签名、ShellCheck、版本、workspace、隐私与审计策略检查）和 `git diff --check` 均退出 0；源码检查复用上表结果，最终提交的完整回归以三项 CI 为准。

原生文件输入形状、file cancel、其他桌面 build、真实引用操作、两端审批与视觉/滚动体验仍为【未核实】或待用户验收。合成 socket 验证请求、问答与 elicitation 的状态撤下，不替代真实入口结论；所有身份、文件与队列测试使用隔离 fixture。

## 待用户验收

安装后分别选择本机与远程机器，检查三个 Provider 的列表、详情、历史分页、搜索、附件、Plan 和子智能体阅读；已有连接、通知配置和管理保护应保持。

本机 Codex 在原生桌面打开同一主线程后连接。输入 `$` / `@`，用上下键和 Enter/Tab 选中引用并发送，确认桌面可见进度；修改占位文字或删除标签应同步撤销引用。目录错误时重试，核对模型/思考强度/Plan、文件/图片、排队、补充与停止。命令/文件请求分别试“允许本次”及可用时的“允许本次会话”；在任一侧处理审批或问答后，另一侧待处理项应消失。隐藏窗口或切换线程后不应恢复旧请求。断线或重启后队列暂停，结果不明需先在 Codex 核对；远程、归档和子线程没有发送入口。详细步骤见 [README](../../README.md#本机-codex-协作) 和[部署手册验收](../cloud-deploy-runbook.md#升级验收)。真实功能、视觉及日常使用由用户确认，安装仅检查成功状态、版本和启动。

## 保留边界

API 协议保持 2，既有 RPC、DTO 与 tombstone 语义不变；新增七个桌面专属命令、可选能力和 NexusHub 自有加密发送记录。保留旧服务回退与有效迁移，移除条件见[兼容与自有数据迁移](../ARCHITECTURE.md#兼容与自有数据迁移)。不直接修改原生会话或队列，不回退到独立 CLI 执行；Keychain、常量时间认证比较、禁止重定向与 systemd 权限边界保持。

资产范围为 macOS ARM64 DMG/checksum、updater archive/signature、latest.json 和 Linux x86_64 API tarball/checksum/signature，共八项；updater 仅映射 darwin-aarch64。首次签名升级由可信客户端验签，先升级 API，再安装正式 App，只做必要部署成功、运行版本与启动检查。正式产物、会话、连接、凭据与共享缓存保留；只清理本轮明确归属的暂存。
