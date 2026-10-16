# NexusHub 当前状态（维护者）

## 当前版本

当前维护版本为 1.3.1。正式发布状态、中文说明与八项资产以 [GitHub Release](https://github.com/lich13/nexushub/releases) 为准，对应提交的完整回归结果见 [CI](https://github.com/lich13/nexushub/actions/workflows/ci.yml)。历史改动与测量记录由 Git 和 Release 保留。

## 当前改动

本轮 1.3.2 基于 `16b8839`（1.3.1），尚未发布或安装。保留已交付的内联引用、UTF-8 区间、目录缓存与重试、原生事件刷新、会话审批门控和固定协议白名单。

- N0/N1：本轮未采集真实桌面输入，按批准范围跳过依赖 live snapshot 的编码修改；既有 `source-derived.json` 仍明确为源码派生 fixture。普通文件兼容前缀、图片原生字段及旧加密队列正文不变。
- N2：审批增加可选只读 review；命令等宽显示，文件 diff 按文件折叠且展开才挂载，权限及未知字段有界展示。保留旧 body 后备、既有审批上限及完整 request/changes 身份；截断不可批准，不增加决策。
- N3：当前连接显示等待处理、执行、发送、排队或空闲状态，队列数量同行；复用原生快照及 RunningIndicator。断连、隐藏、切换后不显示旧状态，不新增 IPC 或高频轮询。
- N4：原生拖放与显式图片粘贴只在 Rust 接收，专用桌面命令只接受接收区和 handle，通用事件监听禁止暴露路径。保留文件身份、引用数量和图片大小校验；整批拒绝无效拖放，隐藏/切换丢弃迟到结果，发送或选择文件期间暂停接收。粘贴图片为原生 owner 可读取的私有文件，归属索引加密；发送前持久化保留标记，只有可证实未提交的草稿可清理，损坏或未知遗留状态保留并停止。详细容量、权限和恢复边界见[架构说明](../ARCHITECTURE.md#本机-codex-桌面协作)。
- N5：协议不支持、owner 不可用和失步提示已核验 build 20586 及同线程重连操作；连接错误放在连接行，保留阅读和草稿，日志仍使用固定分类。
- N6：现有 monitor 仍依据原生 rollout；没有取得“仅 IPC pending 导致漏通知”的可复现证据，因此不扩展通知。此项为【未核实】，不等同于证明无缺口。
- N7：六份当前文档同步实现、安全边界和用户验收步骤。版本与正式产物在收尾阶段统一同步；本轮没有批量升级依赖，macOS 剪贴板复用锁文件已有的系统框架绑定。

## 验证范围

本机必要检查针对 `550e522`、`c0e2aa4`、`6230812` 所含的实际源码执行；之后仅同步文档。未运行原生 App、浏览器功能或视觉验收。完整回归、跨平台构建及 Chromium/WebKit、macOS headed 性能门禁仍待对应提交的现有三项 CI，不能将下表等同于完整 CI 通过。命令入口见 [README 开发](../../README.md#开发)。

| 范围 | 本轮命令与结果 |
|---|---|
| Rust core | `cargo test -p nexushub-core --locked` 最终退出 0：494 个单元测试、226 个集成测试通过；10 个既有 ignored 项未运行。`cargo clippy -p nexushub-core --all-targets --locked -- -D warnings` 退出 0。 |
| Tauri | `cargo test --manifest-path src-tauri/Cargo.toml --locked` 退出 0：101 个单元测试、8 个 entry guard 通过；同 manifest 的 `cargo clippy --all-targets --locked -- -D warnings` 退出 0。私有 NSPasteboard fixture 测试不读写真实用户剪贴板。 |
| 前端 | `corepack pnpm@12.11.2 --dir webui typecheck`、`test`、`build:tauri` 最终退出 0；53 个文件、711 项测试通过，生产构建转换 1,961 个模块。 |
| 契约与配置 | `node scripts/check-wire-types.mjs`、`node --test scripts/test-wire-types.mjs`（8 项）、`python3 scripts/check-versions.py`、`check-workspaces.py`、`test-tauri-build-config.py` 均退出 0。版本检查此时核对的是 1.3.1 manifest，1.3.2 版本同步尚未执行。 |
| 安装、格式与隐私 | `bash scripts/test-install-script.sh` 退出 0，包含 ShellCheck、签名/安装安全、版本、workspace、隐私及审计策略测试。两套 workspace 的 fmt check、`python3 scripts/privacy-check.py --git-objects` 与 `git diff --check` 退出 0；隐私扫描覆盖本轮源码提交和当前文档工作树。 |

新增覆盖包括审批 UTF-8 截断与原有上限、完整请求/diff 绑定、旧 body 回退、状态优先级及旧会话隔离；附件路径/字节注入拒绝、原生拖放整批核验、PNG/TIFF 与剪贴板变化、图像/存储容量、私有权限、身份替换、草稿释放/过期、已提交保留、索引加密/损坏/中断写入；前端普通文本粘贴、附件导入、句柄载荷、隐藏/解绑及迟到结果、发送/选文件期间暂停接收。全部破坏性场景使用隔离 fixture。

本轮并非首次全绿：早期 Rust 编译因有界读取方法歧义及 Tauri Channel 参数类型失败（101），修正后重编通过；测试 fixture 的导入和 HTML 转义断言也作了修正。完整 core 曾因适配层签名快照未同步退出 101，更新契约清单后复验；后续完整运行又发现路径发现测试错误依赖宿主 PATH（101），仅将断言改为核对固定回退顺序。前端全量首次为 708 通过、3 失败（1），原因是设置目录测试缺少附件 hook 隔离 mock；修正后重新运行统一 typecheck、完整 core 和前端测试，结果见上表。没有用局部复验代替这些完整命令的最终结果。

原生文件完整 wire、file cancel、其他桌面 build、真实引用操作、两端审批与视觉/滚动体验仍为【未核实】或待用户验收。协议 2、桌面 owner、旧队列逐字节兼容和结果不明不重发均保留。

## 待用户验收

前提为兼容的本机 Codex 桌面已打开同一主线程。连接后输入 `$` / `@`，选择引用，拖入普通文件或粘贴图片并发送；预期标签仅显示名称，原生桌面可见消息、图片和执行进度，协作区随原生状态变化。普通文本粘贴应保持换行，隐藏或换线程后不应加入迟到附件。

出现命令或文件审批时，核对等宽命令、逐文件折叠 diff 和截断提示；仅支持时显示“允许本次会话”。分别在 NexusHub 或 Codex 完成审批/问答，预期另一侧待处理项撤下；超出显示上限仍需在 Codex 处理。断线后阅读和草稿保持，结果不明项不自动重发；远程、归档和子线程没有发送入口。

真实输入 snapshot、普通文件气泡是否与原生一致、两端审批、OS 拖放与剪贴板交互、视觉及日常使用均待用户验收，合成 socket、私有剪贴板 fixture 和前端 mock 不替代这些结论。详细步骤见 [README](../../README.md#本机-codex-协作) 和[部署手册](../cloud-deploy-runbook.md#升级验收)。正式部署/安装仅检查成功状态、版本和启动。

## 保留边界

API 协议保持 2，既有 RPC、DTO 与 tombstone 语义不变；八个桌面专属命令、可选能力和 NexusHub 自有加密发送记录均不向 Linux RPC 或远程 allowlist 开放。保留旧服务回退与有效迁移，移除条件见[兼容与自有数据迁移](../ARCHITECTURE.md#兼容与自有数据迁移)。不直接修改原生会话或队列，不回退到独立 CLI 执行；Keychain、常量时间认证比较、禁止重定向与 systemd 权限边界保持。

资产范围为 macOS ARM64 DMG/checksum、updater archive/signature、latest.json 和 Linux x86_64 API tarball/checksum/signature，共八项；updater 仅映射 darwin-aarch64。首次签名升级由可信客户端验签，先升级 API，再安装正式 App，只做必要部署成功、运行版本与启动检查。正式产物、会话、连接、凭据、私有附件与共享缓存保留；只清理本轮明确归属的暂存。
