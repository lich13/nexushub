# NexusHub 当前状态（维护者）

## 当前版本

当前维护版本为 1.2.23。正式发布状态、中文说明与八项资产以 [GitHub Release](https://github.com/lich13/nexushub/releases) 为准，对应提交的完整回归结果见 [CI](https://github.com/lich13/nexushub/actions/workflows/ci.yml)。历史改动与测量记录由 Git 和 Release 保留。

## 当前改动

- API 配置加载与绑定前验证回环地址，拒绝时不启动数据库或后台任务，也不写回配置迁移。
- 更新和 Probe 作业使用固定程序及字面参数；完整匹配历史默认命令，保留任务状态、互斥、审计和序列化展示字段。shell_quote 共用一个实现并保留三种既有引用策略。
- Bark 在保存与发送前校验目的地；保留 HTTP 回环，危险或无效地址不投递、不重试，也不替换为默认服务。
- Linux 归档包含版本签名，预装 Minisign 和公钥在解包前验证来源；安装继续原子替换和普通失败回滚，解压权限统一且不保留特殊位。
- 固定 Rust 工具链、双 workspace MSRV、Cargo 锁文件约束与 CI 超时；缓存审计工具和浏览器。审计门禁阻断活动高危、严重、无法判级漏洞与扫描错误，保留原始 JSON 和其他警告。
- 定向修复 Quinn、Rustls、桌面 plist 及必要依赖，WebUI 六项声明精确匹配已有锁定结果；默认 Linux 路径继续假设 ubuntu 宿主用户。

## 验证范围

本机阶段门禁执行根 workspace 与 Tauri 的 fmt、test、clippy，WebUI 类型、单测和生产构建，Chromium/WebKit 功能测试、串行 headed 性能测试，以及契约、ShellCheck、安装、Python 和隐私检查。命令见 [README 开发](../../README.md#开发)；性能门槛和机读结果沿用现有测试，不新增测试框架。

运行时边界、无效命令、签名错误、公钥错误、版本与布局不符、恶意归档权限及安装故障均使用隔离 fixture；临时测试签名密钥运行时生成并清理。核心阶段审计没有活动漏洞，桌面保留 6 项 unmaintained、3 项 unsound 警告，根 workspace 的 1 项 unsound 与桌面重复；最新扫描结果以对应 CI artifact 为准，不等同于无风险承诺。

## 待用户验收

安装后分别选择本机与远程机器，检查三个 Provider 的列表、详情、历史分页、搜索、附件、Plan 和子智能体阅读；已有连接、通知配置和管理保护应保持。

在 Probe 核对已有合法 Bark 目标，并发起测试通知检查设备实收；无效地址应就地失败，不改投默认服务。在“更新与维护”检查两端版本和更新反馈。实际删除与人为修改生产监听、命令或通知地址不作为助手安装验收；真实功能、视觉和日常使用由用户确认。

## 保留边界

API 协议保持 2，DTO、RPC 与桌面命令结构不变，不新增表、设置或界面入口。保留所有 tombstone、旧服务回退和有效迁移；移除条件见[兼容与自有数据迁移](../ARCHITECTURE.md#兼容与自有数据迁移)。不改原生会话、Keychain、常量时间认证比较、禁止重定向与 systemd 权限边界；无证据的其他漏洞和额外重构不实施。

资产范围为 macOS ARM64 DMG/checksum、updater archive/signature、latest.json 和 Linux x86_64 API tarball/checksum/signature，共八项；updater 仅映射 darwin-aarch64。首次签名升级由可信客户端验签，先升级 API，再安装正式 App，只做必要部署成功、运行版本与启动检查。正式产物、会话、连接、凭据与共享缓存保留；只清理本轮明确归属的暂存。
