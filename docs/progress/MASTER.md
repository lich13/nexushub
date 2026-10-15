# NexusHub 当前状态（维护者）

## 当前版本

当前维护版本为 1.2.22。正式发布状态、中文说明与七项资产以 [GitHub Release](https://github.com/lich13/nexushub/releases) 为准，对应提交的完整回归结果见 [CI](https://github.com/lich13/nexushub/actions/workflows/ci.yml)。历史改动与测量记录由 Git 和 Release 保留。

## 当前改动

- 仓库简介与远程机器文案统一为当前产品范围；README 保留用户操作，内部参数和兼容矩阵集中在架构文档。
- 退役命令及适配层别名使用共享表；自有数据库迁移以版本和跨进程锁守门，保留中断标记、秘密清理顺序及业务数据。
- 集中命名缓存预算，补充淘汰、失效和迟到响应覆盖；预算不等于进程 RSS 硬上界。
- 实际 Rust/TypeScript 类型、序列化规则、动态 JSON、适配函数和 schema 纳入契约检查；遗漏字段或映射的负面用例必须失败。
- Codex、Claude、webd 和 Probe 按现有职责机械拆分，使用拆分前生成的脱敏完整响应快照验证等价。
- 部署增加传输校验，安装安全解包、原子替换和普通失败回滚；管理操作补充隔离后的文件身份复核和故障注入测试。
- CI 固定 Action SHA、收敛权限、补发布缓存和非阻塞审计报告；版本、双 Cargo workspace 与 Python 隐私检查进入统一门禁。

## 验证范围

本机阶段门禁执行根 workspace 与 Tauri 的 fmt、test、clippy，WebUI 类型、单测和生产构建，Chromium/WebKit 功能测试、串行 headed 性能测试，以及契约、ShellCheck、安装、Python 和隐私检查。命令见 [README 开发](../../README.md#开发)；性能门槛和机读结果沿用现有测试，不新增测试框架。

管理、迁移、损坏记录、半行、并发、符号链接和安装故障均使用隔离 fixture。密钥与传输检查覆盖错误、日志、审计、恶意回显、HTTPS、重定向和响应大小；保留既有来源 IP 审计。自动测试通过不代表真实 App 功能验收。

## 待用户验收

安装后分别选择本机与远程机器，检查三个 Provider 的列表、详情、连续历史分页、搜索、附件、Plan 和子智能体阅读；刷新、展开工具和切换机器应保留既有位置与隔离行为。

在 Probe 检查已有设置及事件展示，在“更新与维护”检查远程服务标题、隐藏扫描进度、停止与重新扫描。管理操作先核对预览目标和保护原因；实际删除不作为助手安装验收。真实视觉、滚动体感和日常使用由用户确认。

## 保留边界

API 协议保持 2，DTO 语义及正常使用行为保持既有约定，不新增 RPC、表、设置或界面入口。保留所有 tombstone、旧服务回退和有效迁移；移除条件见[兼容与自有数据迁移](../ARCHITECTURE.md#兼容与自有数据迁移)。不实施 RSS 硬限额、自动崩溃恢复、地址脱敏或依赖批量升级，Python 继续保留。

资产范围仍为 macOS ARM64 DMG/checksum、updater archive/signature、latest.json 和 Linux x86_64 API tarball/checksum，共七项；updater 仅映射 darwin-aarch64。先升级 API，再安装正式 App，只做必要部署成功与启动检查。正式产物、会话、连接、凭据与共享缓存保留；只清理本轮明确归属的暂存。
