# NexusHub 当前状态（维护者）

## 当前版本

当前维护版本为 1.2.17。正式发布状态、说明与资产以 [GitHub Release](https://github.com/lich13/nexushub/releases) 为准，对应提交的完整回归结果见 [CI](https://github.com/lich13/nexushub/actions/workflows/ci.yml)。历史改动与交付记录由 Git 和 Release 保留。

## 当前改动

- 三种 Provider 共享有界增量 JSONL 读取，按原生身份定位，复用摘要／正文快照；普通追加不重复解析完整历史。
- Codex、Claude、Grok 和子智能体分开最新窗口轮询与历史取页，以可选来源版本和稳定身份更新历史状态；API 仍为 2，旧客户端与旧服务继续兼容。
- 工作区按需加载、折叠正文延迟挂载、稳定消息和 Markdown 复用渲染、时间线缓存位置；线程首击立即选择。
- 颜色独立于 K／M／G 单位，按实际字节在 100MiB 与 300MiB 分界。未知与部分统计保持原有提示。

## 验证状态

受影响的 Rust、WebUI、Tauri、契约、安装及隐私检查已通过。Chromium 与 WebKit 的历史轮询、颜色和分页检查通过；最新交互样本的展开／滚动／切换 p95 分别为 26.2／26.7／33.1ms、58／36／79ms，均低于 100ms 目标。

同一工具链、同一运行时合成样本的旧／新交替对照中，三种 Provider 的 100／300MiB 热读 p95 均下降超过 99%，大文件冷读未发现超过 10% 的稳定退化。每个热读单元各采样五次；冷读使用新进程并保留系统文件缓存。读取耗时、序列化、传输量、CPU 和峰值内存分别计量，不将读取收益视作完整交互收益。

10MiB 冷读检查和分页覆盖通过；Grok 新分页响应比旧全量响应减少约 84%，Codex 约 3%。70 会话列表的最终旧／新对照各采样五次，冷读 CPU／耗时中位数分别下降约 2%／5%，热读 p95 从 591.2ms 降至 37.4ms。派生摘要通过同长度原地改写并恢复 mtime、原子替换和截断的失效回归，列表、详情及管理保护保持一致。

上述结果来自受控合成样本，不能替代正式 App、远程 API 或真实会话的验收。完整回归由现有三项 CI 执行，包含 Chromium 与 WebKit；发布、安装和真实入口验收须核对同一提交。远程网络耗时与本机读取耗时分别计量，临时文件实际回收空间与写入下降分别记录。

## 保留边界

保留 Codex、Claude Code、Grok、Probe、设置、搜索、工具折叠、Plan、附件、路径、通知与子智能体阅读。Pi、Gotify、网页版和自动 Goal 恢复保持退役，有效迁移、兼容 tombstone、安全检查和回归测试继续保留。

资产范围仍为 macOS ARM64 DMG/checksum、updater archive/signature、latest.json 和 Linux x86_64 API tarball/checksum，共七项；updater 仅映射 darwin-aarch64。先升级 API，再安装正式 App。macOS 包无 Developer ID 签名或 Apple 公证，Minisign 不能替代系统代码签名。
