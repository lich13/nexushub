# NexusHub 当前状态（维护者）

## 版本

目标版本 **1.2.8**，在 `main` 增量开发。当前处于门禁与正式发布前验收阶段，尚未宣称完成安装或部署。

- 三个 Provider：Codex、Claude Code、Grok Build。
- 远程 API 协议：2，先升级服务再安装 App。
- 七项资产：macOS ARM64 DMG/checksum、updater archive/signature、`latest.json`、Linux x86_64 webd tarball/checksum。

## 本版调整

- 线程右键、省略号和键盘操作共用菜单，保留当前详情目标、双击改名与批量操作。
- Codex 普通及归档线程支持预览后永久删除，复核明确终态、身份、引用与文件指纹。
- 隐藏清理按预览候选逐项执行，受保护项跳过，公共故障回滚整批。
- 统一列表卡片宽度；窗口隐藏创建后一次适配工作区，重开和机器切换不反复调整大小。
- Pi 集成及自有队列/配置退役，原生 Pi 程序、用户会话和配置保留。

## 延续能力

当前线程/Provider 搜索、用户指令时间线、工具活动原始顺序、问答气泡、附件、Plan 导出、路径操作、AGENTS.md 折叠与记忆过滤继续保留。本机与远程缓存和写操作隔离，Key 只保存在 Keychain。Bark 继续由各机器独立监测。

## 验证与边界

发布门禁包括 WebUI 类型/单元/构建及 Chromium/WebKit、Rust/Tauri fmt/test/Clippy、契约、迁移、安装、隐私与差异检查。完成三项 CI 才创建 tag。

云端 Rust workspace 测试、Clippy、格式、安装与隐私门禁通过；前端 268 项单元测试、frozen install、类型检查和生产构建通过。Chromium/WebKit 与 macOS Tauri 门禁由对应 CI 执行，正式资产、云端升级和 macOS App 真实入口仍在进行。最终结果更新在本文件，不额外建立验收报告。

初次可达历史审计未发现敏感值，因此沿用增量提交。发布前后继续检查源码、提交与资产；GitHub 保留的不可达对象不属于可达历史扫描覆盖范围。

## 发布说明

中文 Release 固定包含“版本概览、功能调整、问题修复、兼容性与迁移、支持平台与资产、升级提示”。不写入私人主机、路径、真实会话、凭据或测试日志正文。
