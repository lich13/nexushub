#!/usr/bin/env python3
"""Render concise Chinese release notes for GitHub Releases and updater metadata."""

from __future__ import annotations

import argparse
import sys


HIGHLIGHTS: dict[str, list[str]] = {
    "1.3.2": [
        "本机 Codex 协作支持拖入文件与粘贴图片，发送后保留图片供原生历史读取",
        "审批卡按命令和文件整理，补齐协作状态与连接提示，保持原生桌面执行和协议 2。",
    ],
    "1.3.1": [
        "本机 Codex 支持输入 $ / @ 选择技能、应用和插件，保留原生桌面执行与加密队列",
        "改进目录重试、协作进度刷新与协议错误提示，支持已核实的命令和文件会话审批。",
    ],
    "1.3.0": [
        "本机 Codex 主线程支持新建、续聊、排队、补充与停止，仍由原生桌面执行",
        "接入模型、模式、引用及待处理请求，保留远程与其他 Provider 的现有阅读和管理功能。",
    ],
    "1.2.23": [
        "限制 API 回环监听、受控更新命令和 Bark 地址，保持现有功能与协议兼容",
        "Linux 更新增加分离签名验证，规范解包权限并强化依赖审计与构建门禁。",
    ],
    "1.2.22": [
        "加固迁移、管理操作和安装回滚，保留现有会话与兼容行为",
        "整理读取模块、资源预算与文档，统一远程机器文案并补强契约检查。",
    ],
    "1.2.21": [
        "自动上下文合并为一行默认收起，保留真实请求和附件，搜索排除自动字段",
        "缩窄时间线预览，补齐全部用户记录的历史定位与双向分页。",
    ],
    "1.2.20": [
        "修复 Grok 闲置线程因历史回合未闭合而被误保护的问题",
        "隐藏线程自动分批扫描，显示进度、支持停止，并限制每页显示数量。",
    ],
    "1.2.19": [
        "已回答问题保留原提问、选项和回答，搜索命中的提问文字可直接阅读",
        "延续长线程性能优化、历史分页和 100M／300M 线程颜色规则。",
    ],
    "1.2.18": [
        "搜索可展开并定位折叠的旧问答、工具结果与 AGENTS.md 指令。",
        "保留其他手动折叠选择，修复成功定位后仍显示旧失败提示的问题。",
    ],
    "1.2.17": [
        "减少长线程重复读取和历史轮询开销，改善页面切换、工具展开与滚动响应",
        "Grok 支持上滚分页，三种 Provider 按 100M／300M 统一线程大小颜色。",
    ],
    "1.2.16": [
        "向上滚动自动加载较早消息，保留当前阅读位置，搜索与分页协同工作。",
        "Codex、Claude Code、Grok 线程列表显示 K／M／G 大小，支持本机和腾讯云。",
    ],
    "1.2.15": [
        "Bark 网络异常后有限重试，重启继续未确认分段，减少漏推。",
        "细分安全错误与中文投递状态，统一 NexusHub 测试文案并移除旧配置导入。",
    ],
    "1.2.14": [
        "移除 Gotify 设置和推送能力，通知统一使用 Bark。",
        "升级清理旧通道配置与投递数据，保留 Bark 去重状态和 AGENTS.md 完整折叠修复。",
    ],
    "1.2.13": [
        "修复 AGENTS.md 原生指令只折叠开头的问题，完整正文默认收起，保留手动展开选择。",
    ],
    "1.2.12": [
        "精简子智能体汇总和清理面板，移除三处重复说明，保留状态数量与实际操作结果。",
        "延续 1.2.11 的开始活动、统一任务名称、直属统计及内部页面标记过滤。",
    ],
    "1.2.11": [
        "修复 Codex 子智能体开始活动漏显示及任务名称不一致，开始记录在任务完成后仍保留。",
        "线程详情新增直属子智能体状态汇总，可从列表进入详情并返回；父线程静止时仍更新数量。",
        "默认隐藏内部页面标记，显示、复制、导出和搜索同步清理，保留代码与引用示例。",
    ],
    "1.2.10": [
        "修复切换远程机器后列表和设置持续加载的问题，钥匙串访问改为后台串行执行。",
        "增加凭据读取超时反馈，保留连接事务与机器切换保护，已有连接无需重建。",
    ],
    "1.2.9": [
        "统一 Codex、Claude Code 和 Grok 的紧凑阅读布局，正文与活动摘要对齐，复制按钮不再占据额外一行。",
        "新增 Codex 子智能体活动与右侧只读详情，支持嵌套返回、分页、工具记录和附件。",
        "修复 Hook 集成测试的 Cargo 二进制路径依赖，保留真实 CLI 和通知去重验证。",
    ],
    "1.0.0": [
        "Codex 支持批量归档、恢复与删除，Grok 和 Pi 支持批量删除。",
        "扩展 Bark 完成和失败通知，并移除已退役的日志维护入口。",
        "完成隐私清理和仓库结构整理，远程部署参数继续从仓库外提供。",
    ],
    "1.1.0": [
        "移除 NexusHub 自有 Goal 与自动恢复链路。",
        "Codex、Grok、Pi 统一显示运行状态，并将连续命令整理为可折叠执行组。",
        "发布资产收敛为 macOS App、updater 和 Linux webd 服务包。",
    ],
    "1.1.1": [
        "修复 Grok 原生 Execute 工具未被归入命令执行组的问题。",
        "连续执行活动可合并查看，保留每项工具结果。",
    ],
    "1.1.2": [
        "改进 Grok 工具活动识别和连续命令分组。",
        "回复操作栏与双击改名交互更接近 Codex 桌面端。",
    ],
    "1.1.3": [
        "修复 Grok 连续工具折叠和长会话滚动空白。",
        "为 Codex 与 Grok Plan 增加复制和 Markdown 下载。",
    ],
    "1.1.4": [
        "隐藏结构化记忆元数据，显示、复制和导出使用同一清理规则。",
        "AGENTS.md 内容默认折叠，并保留用户手动展开状态。",
    ],
    "1.1.5": [
        "为 Codex 普通提问增加 Bark 提醒。",
        "改进 Markdown 文件路径复制、Finder 定位和更新维护设置。",
    ],
    "1.1.6": [
        "统一 Codex、Grok、Pi 的用户消息气泡和附件展示。",
        "保留原始换行与 Markdown 字符，图片支持按需预览。",
    ],
    "1.1.7": [
        "恢复历史线程中文本与工具活动的原始交错顺序。",
        "完成工具组默认收起、活动状态和滚动跟随优化。",
    ],
    "1.1.8": [
        "结构化问答显示为问题引用与答案气泡。",
        "用户消息中的 AGENTS.md 指令默认折叠并支持独立复制答案。",
    ],
    "1.1.9": [
        "识别 request_user_input_async 的未回答问题并发送 Bark 提醒。",
        "统一 Hook、后台 monitor 和去重记录，异步确认不会误清除待回答问题。",
    ],
    "1.2.0": [
        "下线网页界面，改为由 macOS App 通过 API Key 管理远程机器。",
        "增加本机与远程机器快速切换，并隔离各机器的查询和操作状态。",
    ],
    "1.2.1": [
        "修复 Plan 在 macOS WebKit 中下载停滞，改用原生文件保存。",
        "修复 Grok 改名进程在服务只读隔离下无法正常工作的路径。",
    ],
    "1.2.2": [
        "修复 Grok 已结束线程仍显示运行中的问题。",
        "统一本机 App 与远程服务的更新面板和按钮状态。",
    ],
    "1.2.3": [
        "新增 Claude Code 会话读取、搜索、附件查看和安全管理。",
        "未知格式或活动状态不明确时保持只读，腾讯云不安装 Claude。",
    ],
    "1.2.4": [
        "新增当前线程和当前 Provider 的会话搜索。",
        "增加 Codex 风格时间线，可定位消息、Plan、附件和工具组。",
    ],
    "1.2.5": [
        "修正桌面安装包的版本信息，使应用显示与发布版本一致。",
        "修复服务器更新在 systemd 只读隔离下无法写入的问题。",
    ],
    "1.2.6": [
        "修复本机与腾讯云切换后更新状态和能力缓存串机。",
        "统一本机 App 与腾讯云服务的更新反馈，保留远程 API Key 和监测配置。",
    ],
    "1.2.8": [
        "线程右键直接打开操作菜单，Codex 普通与归档线程支持预览后永久删除。",
        "隐藏线程清理逐项跳过受保护项目，修复列表宽度并优化一次性启动窗口适配。",
        "移除 NexusHub 的 Pi 集成，保留原生 Pi 程序与用户数据；远程 API 协议升级为 2。",
    ],
    "1.2.7": [
        "左侧时间线只标记用户发送指令的位置，滚动时高亮对应指令；助手回复和工具内容仍可搜索定位。",
        "修复 Claude Code 新版辅助记录被误报为读取错误的问题，兼容性提醒与致命错误分别显示，安全管理限制继续保留。",
        "修复 Claude Code 完成 Bark 漏推，支持空结果回退；工具执行中、取消或已有新指令时不误报完成，升级不补推历史通知。",
    ],
}


def highlights(version: str) -> list[str]:
    return HIGHLIGHTS.get(version, ["修复问题并改进稳定性。"])


def render(version: str, *, updater: bool = False) -> str:
    if updater:
        return f"NexusHub {version}：" + "；".join(highlights(version)[:2])

    if version == "1.3.2":
        sections = {
            "版本概览": ["本版完善本机 Codex 协作的附件输入、审批阅读和状态呈现；对话执行、进度、审批与问答仍由原生桌面持有。"],
            "功能调整": [
                "文件可拖入协作输入区，图片可直接粘贴；保留原生文件选择器及 $ / @ 技能、应用和插件引用。",
                "审批命令使用等宽文本，文件 diff 按文件折叠并在展开后加载；协作区显示执行、待处理请求和队列状态。",
            ],
            "问题修复": [
                "审批投影有界处理大内容，保留完整请求与文件变化身份；内容截断或无法完整审阅时仍需在 Codex 处理，不放宽批准条件。",
                "附件只从原生拖放或明确粘贴进入，界面不接收文件路径或图片字节；切换、隐藏、发送和选择文件时隔离迟到结果。",
                "协议不兼容、owner 不可用与状态失步在连接处显示固定提示，注明已核验 build 20586，保留历史阅读与草稿。",
            ],
            "兼容性与迁移": [
                "API 协议保持 2，新增附件命令仅供本机 macOS；只读审批字段保持可选，Linux、远程、归档与子线程不开放发送。",
                "保留旧加密队列正文、会话审批门控及结果不明不重发。粘贴图片保存在私有文件中，归属索引加密；交给发送队列后保留，避免清理破坏原生历史。",
                "普通文件继续使用已有兼容编码；本轮未取得原生文件完整输入快照，不扩展未核实的审批决策，也不增加缺少复现证据的 IPC 通知。",
            ],
            "支持平台与资产": [
                "macOS ARM64：DMG／校验文件、updater 压缩包／签名及仅映射 darwin-aarch64 的 latest.json。",
                "Linux x86_64：API 服务包、SHA-256 校验文件与 .sig 分离签名；共八项发布资产。",
            ],
            "升级提示": [
                "先升级远程 API，再安装 macOS App；保留配置、凭据、会话与私有附件。",
                "在原生桌面打开同一线程并连接，使用 $ / @、拖放或粘贴发送，检查桌面进度、审批 diff 及任一侧处理后的状态同步。真实操作、视觉与日常体验待用户验收。",
                "macOS 包没有 Developer ID 签名或 Apple 公证；Minisign 用于验证归档来源，不能替代系统代码签名。",
            ],
        }
        lines = [f"# NexusHub {version}", ""]
        for heading, paragraphs in sections.items():
            lines.extend([f"## {heading}", ""])
            lines.extend(f"- {paragraph}" for paragraph in paragraphs)
            lines.append("")
        return "\n".join(lines)

    if version == "1.3.1":
        sections = {
            "版本概览": ["本版完善本机 Codex 桌面协作的引用输入、目录读取、进度同步和审批体验；对话仍由原生桌面执行。"],
            "功能调整": [
                "输入 $ 搜索技能、@ 搜索应用与插件；上下键选择、Enter/Tab 插入、Escape 关闭，所选引用显示可读占位文字与可移除标签。",
                "命令和文件审批仅在当前请求声明支持时显示“允许本次会话”；权限请求仍限本回合，不增加永久授权。",
            ],
            "问题修复": [
                "目录读取不再长时间占用缓存锁，同源并发查询合并；超时可重试，应用或插件单侧失败保留另一侧结果。",
                "校验引用的身份和 UTF-8 区间，修复插件显示名与原生名称不一致的问题，保留中文和 emoji 前后的正确占位位置。",
                "协作事件合并刷新当前线程，慢读取不被反复取消；隐藏、切换线程与断开后丢弃旧响应，并保留回合结束后的最终刷新。",
                "协议不兼容、线程未由桌面持有、状态失步和超时使用清晰的固定提示，失败时仍可阅读历史。",
            ],
            "兼容性与迁移": [
                "API 协议保持 2，引用区间与会话审批使用可选字段；协作仍仅供本机 macOS 主线程，远程和其他 Provider 的边界保持不变。",
                "保留旧加密队列正文格式与幂等身份，结果不明不自动重发；升级保留配置、凭据、通知状态及原生会话。",
                "协议依据桌面 build 20586 源码与合成测试核对。普通文件保留现有兼容编码；未核实的原生文件编码、文件 cancel 和其他桌面版本不宣称已支持。",
            ],
            "支持平台与资产": [
                "macOS ARM64：DMG／校验文件、updater 压缩包／签名及仅映射 darwin-aarch64 的 latest.json。",
                "Linux x86_64：API 服务包、SHA-256 校验文件与 .sig 分离签名；共八项发布资产。",
            ],
            "升级提示": [
                "先升级远程 API，再安装 macOS App。安装后在原生桌面打开同一主线程，连接后用 $ / @ 发送，检查桌面进度和两侧审批同步。",
                "真实引用展示、两侧操作、视觉与滚动体验待用户验收；本轮合成测试不替代实机结论。",
                "macOS 包没有 Developer ID 签名或 Apple 公证；Minisign 用于验证归档来源，不能替代系统代码签名。",
            ],
        }
        lines = [f"# NexusHub {version}", ""]
        for heading, paragraphs in sections.items():
            lines.extend([f"## {heading}", ""])
            lines.extend(f"- {paragraph}" for paragraph in paragraphs)
            lines.append("")
        return "\n".join(lines)

    if version == "1.3.0":
        sections = {
            "版本概览": ["本版接入本机 Codex 桌面协作，在 NexusHub 中继续对话，由原生桌面执行。现有三个 Provider 的阅读、通知与管理功能继续保留。"],
            "功能调整": [
                "本机普通主线程可新建、续聊、排队发送、补充当前回合或停止；模型、推理强度和模式沿用原生桌面可用选项。",
                "支持选择文件、图片及可用的技能、应用和插件引用；已支持的问题和审批可在面板处理，无法安全表达的请求回到原生桌面处理。",
                "NexusHub 独立保存待发送队列，在线程空闲且原生桌面持有该线程时逐项交付；不覆盖原生桌面的队列。",
            ],
            "问题修复": [
                "兼容当前原生 App 内置 CLI 的目录布局，处理连接取消、迟到响应及机器切换，避免旧线程状态覆盖当前页面。",
                "发送前重新检查线程状态、引用文件身份和请求内容；结果不确定的发送不自动重试，防止重复提交。",
                "断开或重启后待发送项目保留为暂停状态，恢复前需确认；停止当前回合也会暂停尚未交付的项目。",
            ],
            "兼容性与迁移": [
                "API 协议保持 2，新增协作命令仅供本机 macOS App 使用；远程、归档、子线程及 Claude Code、Grok 不提供协作输入。",
                "队列存储在 NexusHub 自有数据库并使用独立钥匙串密钥加密；升级保留既有连接、凭据、通知状态与原生会话。",
                "原生桌面协议不兼容、线程未由桌面持有或身份无法核验时停止协作，不回退为后台 CLI 执行。",
            ],
            "支持平台与资产": [
                "macOS ARM64：DMG／校验文件、updater 压缩包／签名及仅映射 darwin-aarch64 的 latest.json。",
                "Linux x86_64：API 服务包、SHA-256 校验文件与 .sig 分离签名；共八项发布资产。",
            ],
            "升级提示": [
                "先升级远程 API，再安装 macOS App。使用协作前打开兼容的原生 Codex 桌面并打开目标线程；新建空线程在首次发送前没有聊天内容。",
                "首次使用可能出现系统钥匙串授权。安装后请用专用线程验证发送、排队、补充、停止、引用及审批；视觉、滚动和日常使用待用户验收。",
                "macOS 包没有 Developer ID 签名或 Apple 公证；Minisign 用于验证归档来源，不能替代系统代码签名。",
            ],
        }
        lines = [f"# NexusHub {version}", ""]
        for heading, paragraphs in sections.items():
            lines.extend([f"## {heading}", ""])
            lines.extend(f"- {paragraph}" for paragraph in paragraphs)
            lines.append("")
        return "\n".join(lines)

    if version == "1.2.23":
        sections = {
            "版本概览": ["本版加固现有监听、更新链与工程检查，保留既有阅读、通知及管理流程。"],
            "功能调整": [
                "不新增用户功能、RPC、设置或界面入口；Codex、Claude Code、Grok、Probe 和现有管理保护保持原有边界。",
                "开发与 CI 固定 Rust 1.99.0，最低 Rust 声明为 1.88；前端依赖采用锁文件中的精确版本。",
            ],
            "问题修复": [
                "拒绝非回环 API 监听，在数据库及后台任务启动前返回固定错误。更新与 Probe 任务使用固定程序和字面参数，不执行自定义 shell 字符串。",
                "Bark 保存与投递共用地址校验，拒绝危险 IP、数字 IP 别名和 URL 凭据；保留 HTTP 回环，非法配置不会改用默认服务发送。",
                "Linux 更新先验证 SHA-256 和 Minisign 分离签名，再解包并核对版本、平台及布局；解压不继承特殊位或 world-writable 权限。",
                "CI 增加超时、--locked 和工具缓存；高危、严重及无法判级的活动漏洞或扫描失败阻断，低中危与维护类告警继续报告。定向修复相关 Rust 依赖。",
            ],
            "兼容性与迁移": [
                "API 协议保持 2，RPC、桌面命令、DTO 与 tombstone 不变；保留 Keychain、认证比较、禁止重定向和 systemd 隔离。",
                "配置只允许回环监听；更新任务接受现行及完整匹配的历史默认命令，其他自定义 shell 配置会明确拒绝。",
                "不改写原生会话。安装普通失败恢复本轮替换文件，不回滚业务数据库；显式本地 --archive/--binary 运维方式保留。",
            ],
            "支持平台与资产": [
                "macOS ARM64：DMG／校验文件、updater 压缩包／签名及仅映射 darwin-aarch64 的 latest.json。",
                "Linux x86_64：API 服务包、SHA-256 校验文件与 .sig 分离签名；共八项发布资产。",
            ],
            "升级提示": [
                "部署客户端与服务器须预装 Minisign。首次从 1.2.22 或更早版本升级，先由可信客户端验证正式包，再安装新的验签器和公钥；不降级为仅 SHA-256。",
                "先升级远程 API，再安装 macOS App；保留现有配置、连接、凭据、通知状态和会话。正常阅读、通知、更新反馈与日常使用待用户验收。",
                "macOS 包没有 Developer ID 签名或 Apple 公证；Minisign 用于验证归档来源，不能替代系统代码签名。",
            ],
        }
        lines = [f"# NexusHub {version}", ""]
        for heading, paragraphs in sections.items():
            lines.extend([f"## {heading}", ""])
            lines.extend(f"- {paragraph}" for paragraph in paragraphs)
            lines.append("")
        return "\n".join(lines)

    if version == "1.2.22":
        sections = {
            "版本概览": ["本版集中维护与加固现有能力，改善工程可维护性与升级可靠性。"],
            "功能调整": [
                "远程机器与服务使用中性名称；保留 Codex、Claude Code、Grok、Probe、设置及现有阅读和管理入口。",
                "整理 Provider 读取、CLI、Hook 和 Probe 展示模块，集中资源预算并补充兼容说明。",
            ],
            "问题修复": [
                "退役数据迁移使用版本守门及跨进程锁，重复开库不再重复扫描退役记录；中断后保留安全接续。",
                "管理操作在隔离后重新核验文件身份与指纹，补充索引、数据库提交和进程终止故障覆盖。",
                "部署校验传输内容，安装拒绝危险归档并原子替换程序；普通安装失败恢复本轮替换文件。",
                "补强真实 DTO、适配层、版本与隐私检查，固定 CI Action 引用并输出非阻塞依赖审计报告。",
            ],
            "兼容性与迁移": [
                "API 协议保持 2，现有 DTO 语义不变；不新增 RPC、数据库表、设置项或 UI 入口。",
                "保留协议 tombstone、旧服务回退、连接、Bark、通知状态和原生会话；仅迁移 NexusHub 自有数据。",
                "缓存预算不等于进程 RSS 硬上界；安装回滚不回滚业务数据库，也不新增崩溃自动恢复。",
            ],
            "支持平台与资产": [
                "macOS ARM64：DMG／校验文件、updater 压缩包／签名及仅映射 darwin-aarch64 的 latest.json。",
                "Linux x86_64：API 服务包与校验文件；共七项发布资产。",
            ],
            "升级提示": [
                "先升级远程 API，再安装本机 App；安装前核对 SHA-256 与 updater 签名。保留现有连接、凭据及会话。",
                "安装后检查本机与远程的阅读、分页、搜索、附件、Probe 与管理预览。真实功能、视觉及日常体感待用户验收。",
                "macOS 包没有 Developer ID 签名或 Apple 公证；Minisign 用于验证 updater 来源，不能替代系统代码签名。",
            ],
        }
        lines = [f"# NexusHub {version}", ""]
        for heading, paragraphs in sections.items():
            lines.extend([f"## {heading}", ""])
            lines.extend(f"- {paragraph}" for paragraph in paragraphs)
            lines.append("")
        return "\n".join(lines)

    if version == "1.2.21":
        sections = {
            "版本概览": ["本版整理自动注入上下文与附件说明，并修复时间线过宽、较早记录无法定位的问题。"],
            "功能调整": [
                "每条用户消息最多一行“上下文”，默认收起，展开时按原顺序显示；附件和真实请求正常保留，纯上下文不产生空气泡或时间线刻度。",
                "主线程、子线程和三个 Provider 使用完整用户消息索引，可直接定位尚未加载的较早记录，再向前后分页或返回最新内容。",
                "左侧刻度与悬停预览缩窄，移除重复提示；大量记录按需挂载，支持滚动和键盘访问。",
            ],
            "问题修复": [
                "同时识别新旧附件请求标题，保留正文换行并按原生身份去重；临时附件失效不影响请求显示。",
                "搜索排除环境及自动包装，保留真实正文、问题回答、附件名和 AGENTS.md 指令定位；代码、引用及不完整结构保持原文。",
                "修复 Grok 大历史只读取末尾 64MiB 导致早期记录缺失的问题，冷读使用完整流式解析并复用增量缓存。",
            ],
            "兼容性与迁移": [
                "API 协议保持 2，详情新增字段均为可选；新客户端连接旧服务时整理原始包装并回退逐页加载，不新增 RPC、数据库表或客户端路径权限。",
                "保留原生会话、管理保护、连接、附件、Plan、通知和 100M／300M 颜色规则；同步已批准的开发工具链与锁文件。",
            ],
            "支持平台与资产": [
                "macOS ARM64：DMG／校验文件、updater 压缩包／签名及仅映射 darwin-aarch64 的 latest.json。",
                "Linux x86_64：API 服务包与校验文件；共七项发布资产。",
            ],
            "升级提示": [
                "先升级远程 API，再安装本机 App。安装前核对校验和与 updater 签名。",
                "安装后检查上下文展开、AGENTS.md 搜索、最早时间线记录、双向分页与返回最新；真实视觉、滚动体验和日常操作待用户验收。",
                "macOS 包没有 Developer ID 签名或 Apple 公证；Minisign 用于验证 updater 来源，不能替代系统代码签名。",
            ],
        }
        lines = [f"# NexusHub {version}", ""]
        for heading, paragraphs in sections.items():
            lines.extend([f"## {heading}", ""])
            lines.extend(f"- {paragraph}" for paragraph in paragraphs)
            lines.append("")
        return "\n".join(lines)

    if version == "1.2.20":
        sections = {
            "版本概览": ["本版修复 Grok 闲置线程误保护与隐藏线程数量较多时的扫描卡顿。"],
            "功能调整": [
                "隐藏线程自动分批扫描，显示准备、扫描、完成或停止状态；候选与逐项结果每页最多显示 100 项。",
                "扫描可随时停止，保留已显示结果；全部扫描完成后才能确认清理，重新扫描会建立新快照。",
            ],
            "问题修复": [
                "Grok 在原生登记与进程身份确认无占用、记录完整有效时，允许历史回合未闭合的线程进入删除预览；仍打开、状态无法核验或记录损坏的线程继续受保护。",
                "隐藏扫描共用数据库元数据、索引和文件引用，流式验证历史；扫描及删除预览移入有界后台执行，减少重复读取与界面阻塞。",
                "Markdown 复用完整转换结果，减少切换线程和重复展开时的处理，保留链接安全与文件路径行为。",
            ],
            "兼容性与迁移": [
                "API 协议保持 2，不新增数据库表；旧客户端仍可单次扫描，新客户端连接旧服务时沿用旧行为。",
                "删除继续绑定预览候选与指纹，并在执行前复核身份、占用和文件变化；保留现有会话、连接、Bark 与通知状态。",
            ],
            "支持平台与资产": [
                "macOS ARM64：DMG／校验文件、updater 压缩包／签名及仅映射 darwin-aarch64 的 latest.json。",
                "Linux x86_64：API 服务包与校验文件；共七项发布资产。",
            ],
            "升级提示": [
                "先升级远程 API，再安装本机 App。安装前核对校验和与 updater 签名。",
                "安装后可在 Grok 打开原先误保护线程的删除预览，并在“更新与维护”检查扫描进度、翻页、停止和重新扫描。",
                "macOS 包没有 Developer ID 签名或 Apple 公证；Minisign 用于验证 updater 来源，不能替代系统代码签名。",
            ],
        }
        lines = [f"# NexusHub {version}", ""]
        for heading, paragraphs in sections.items():
            lines.extend([f"## {heading}", ""])
            lines.extend(f"- {paragraph}" for paragraph in paragraphs)
            lines.append("")
        return "\n".join(lines)

    if version == "1.2.19":
        sections = {
            "版本概览": ["本版补齐已回答问题的历史展示，延续长线程性能优化和 100M／300M 线程颜色规则。"],
            "功能调整": ["已回答问题同时显示原提问、选项及说明和已有回答，全部保持只读。"],
            "问题修复": ["修复搜索命中历史提问后虽然定位成功，卡片却只显示回答、看不到命中文字的问题。"],
            "兼容性与迁移": [
                "API 协议保持 2；复用已有问题记录与稳定位置，不改变问题状态、原生会话或通知去重。",
                "保留增量读取、历史分页、附件、Plan、子智能体、管理保护及已有连接配置。",
            ],
            "支持平台与资产": [
                "macOS ARM64：DMG／校验文件、updater 压缩包／签名及仅映射 darwin-aarch64 的 latest.json。",
                "Linux x86_64：API 服务包与校验文件；共七项发布资产。",
            ],
            "升级提示": [
                "先升级远程 API，再安装本机 App。安装前核对校验和与 updater 签名。",
                "macOS 包没有 Developer ID 签名或 Apple 公证；Minisign 用于验证 updater 来源，不能替代系统代码签名。",
            ],
        }
        lines = [f"# NexusHub {version}", ""]
        for heading, paragraphs in sections.items():
            lines.extend([f"## {heading}", ""])
            lines.extend(f"- {paragraph}" for paragraph in paragraphs)
            lines.append("")
        return "\n".join(lines)

    if version == "1.2.18":
        sections = {
            "版本概览": ["本版修复历史搜索定位，延续长线程性能优化和 100M／300M 线程颜色规则。"],
            "功能调整": ["搜索命中旧问答、工具结果或 AGENTS.md 指令时自动展开目标；再次搜索手动关闭的目标也可重新展开。"],
            "问题修复": [
                "修复已加载的旧问答因界面历史折叠而无法定位，以及关闭的工具组内结果没有正文锚点的问题。",
                "定位命中的具体消息或命令，保留其他折叠选择和分页位置，并清除已失效的搜索失败提示。",
            ],
            "兼容性与迁移": [
                "API 协议保持 2；保留增量读取、历史分页、通知、附件、Plan、子智能体和管理保护。",
                "无需重建连接或迁移原生会话；凭据、配置、通知游标及 Bark 去重状态继续保留。",
            ],
            "支持平台与资产": [
                "macOS ARM64：DMG／校验文件、updater 压缩包／签名及仅映射 darwin-aarch64 的 latest.json。",
                "Linux x86_64：API 服务包与校验文件；共七项发布资产。",
            ],
            "升级提示": [
                "先升级远程 API，再安装本机 App。安装前核对校验和与 updater 签名。",
                "macOS 包没有 Developer ID 签名或 Apple 公证；Minisign 用于验证 updater 来源，不能替代系统代码签名。",
            ],
        }
        lines = [f"# NexusHub {version}", ""]
        for heading, paragraphs in sections.items():
            lines.extend([f"## {heading}", ""])
            lines.extend(f"- {paragraph}" for paragraph in paragraphs)
            lines.append("")
        return "\n".join(lines)

    if version == "1.2.17":
        sections = {
            "版本概览": ["本版优化长线程与多会话列表的读取、页面切换和滚动，统一线程大小颜色分级。"],
            "功能调整": [
                "Codex、Claude Code、Grok 和子智能体详情分开刷新最新内容与加载较早历史；Grok 现在也支持持续上滚自动分页。",
                "三种 Provider 在本机与腾讯云统一按实际字节分色：小于 100M 为绿色，100M 至不足 300M 为琥珀色，300M 及以上为红色，1M = 1024² 字节。",
                "K／M／G 数值格式、准确字节提示和卡片布局保持原有规则；未知为灰色，部分结果保留 ≥ 和统计不完整提示。",
            ],
            "问题修复": [
                "减少未变化会话的重复解析、详情查询连带扫描列表，以及多会话列表反复淘汰正文缓存的开销。",
                "静态旧页不再随每次轮询重复请求；较早工具结果、待回答问题和子智能体状态仍会更新。机器或线程切换后丢弃迟到响应。",
                "线程首击立即选择，工作区按需加载，折叠正文展开时再渲染；缓存 Markdown 解析与时间线位置，保留搜索定位、复制、导出和滚动锚点。",
            ],
            "兼容性与迁移": [
                "API 协议保持 2，不新增公共命令或数据库表；分页和历史变更信息均为可选扩展，旧客户端与旧服务继续兼容。",
                "保留 Codex、Claude Code、Grok、Probe、设置、通知、附件和管理保护；文件替换、截断、原地改写及身份变化会使相应缓存失效。",
                "窗口隐藏时暂停界面轮询，后台 monitor 独立运行。整理当前文档和重复实现，原生会话、连接、凭据及 Bark 配置继续保留。",
            ],
            "支持平台与资产": [
                "macOS ARM64：DMG 安装包与 SHA-256 校验文件、updater 压缩包与签名。",
                "Linux x86_64：API 服务包与 SHA-256 校验文件；latest.json 仅映射 darwin-aarch64，共七项资产。",
            ],
            "升级提示": [
                "先升级远程 API，再安装本机 App。首次打开大文件仍需读取正文，后续刷新和历史阅读会复用缓存。",
                "安装前核对校验和及 updater 签名。macOS 包没有 Developer ID 签名或 Apple 公证，Minisign 不能替代系统代码签名。",
            ],
        }
        lines = [f"# NexusHub {version}", ""]
        for heading, paragraphs in sections.items():
            lines.extend([f"## {heading}", ""])
            lines.extend(f"- {paragraph}" for paragraph in paragraphs)
            lines.append("")
        return "\n".join(lines)

    if version == "1.2.16":
        sections = {
            "版本概览": ["本版改进长线程阅读：上滚即可继续查看历史，列表同时显示会话占用大小。"],
            "功能调整": [
                "Codex 主线程、Claude Code 和 Codex 子智能体详情在向上滚动到顶部附近时，自动逐页加载较早消息；首次打开仍定位到最新内容。",
                "三种 Provider 的线程大小放在原状态行右侧，按 1024 换算为 K／M／G，分别使用绿色、琥珀色和红色；悬停或键盘聚焦可查看准确字节数及统计范围。",
                "Codex 统计 rollout 文件；Claude Code 统计主 JSONL 及确认归属的会话专属目录；Grok 统计会话专属目录。本机与腾讯云采用相同口径。",
            ],
            "问题修复": [
                "取消日常阅读中的手动加载按钮，加载失败后才显示重试入口；历史耗尽时不保留空白区域。",
                "加载较早消息时保持可见内容及其像素位置，轮询追加、工具展开和继续滚动不会把阅读位置拉回底部。",
                "搜索与滚动共用分页协调，避免重复请求；轮询保留已加载历史及最早游标，机器或线程切换后丢弃旧响应。",
            ],
            "兼容性与迁移": [
                "API 协议保持 2，不新增公共命令或数据库表；现有会话、连接、凭据及 Bark 配置继续使用。",
                "目录在后台有界统计，缓存最长 30 秒刷新；不扫描项目工作目录或共享存储，不读取外部附件，不跟随符号链接。",
                "未知或旧服务缺少大小字段时显示 —，部分统计用 ≥ 标记；文件替换或截断会使旧缓存失效。",
            ],
            "支持平台与资产": [
                "macOS ARM64：DMG 安装包与 SHA-256 校验文件、updater 压缩包与签名。",
                "Linux x86_64：API 服务包与 SHA-256 校验文件；latest.json 仅映射 darwin-aarch64，共七项资产。",
                "不提供 Linux 桌面包、网页版或安卓安装包。",
            ],
            "升级提示": [
                "先升级远程 API，再安装本机 App；打开较长线程向上阅读，即可触发历史加载。目录大小可能在下一次列表刷新后显示。",
                "大小表示会话存储的逻辑字节数，不代表项目目录体积或删除后必然释放的磁盘空间。",
                "安装前核对校验和及 updater 签名。macOS 包没有 Developer ID 签名或 Apple 公证，Minisign 不能替代系统代码签名。",
            ],
        }
        lines = [f"# NexusHub {version}", ""]
        for heading, paragraphs in sections.items():
            lines.extend([f"## {heading}", ""])
            lines.extend(f"- {paragraph}" for paragraph in paragraphs)
            lines.append("")
        return "\n".join(lines)

    if version == "1.2.15":
        sections = {
            "版本概览": ["本版改善 Bark 在网络波动后的投递，增加持久化有限重试、分段恢复和明确的中文反馈。"],
            "功能调整": [
                "每个分段最多尝试 3 次，两次重试通常等待 15 秒、60 秒并加入 ±20% 抖动；从首次投递起最长保留 10 分钟，尊重服务端 Retry-After。",
                "重启后继续未确认的分段，已成功分段不重发；Hook 保持短等待，重试由后台 monitor 处理。",
                "界面区分等待重试、重试中、Bark 已受理、发送失败、结果未确认和已跳过，并显示次数及分段进度。",
            ],
            "问题修复": [
                "网络故障不再统一折叠为 request_error；按 DNS、连接、TLS、超时、响应读取、HTTP 或 Bark 拒绝分类，诊断不记录地址、Key 或通知正文。",
                "每次重试重新核对事件、开关与投递目标，等待期间修改配置也会停止失效通知；恢复成功更新原事件。",
                "测试统一使用“NexusHub 推送测试”和本机／腾讯云来源正文，等待真实受理结果；移除旧程序配置导入实现与旧产品文案。",
            ],
            "兼容性与迁移": [
                "API 协议保持 2，不新增公共命令或数据库表；保留现有会话、Keychain 连接、管理员 API Key、Bark 凭据及通知基线。",
                "升级不补发历史失败或终态未知结果。新队列加密保存正文，完成或停止后清除；凭据错误、证书验证失败、重定向及已知永久 APNs 错误不重试。",
                "本版优先减少漏推，结果不明时也有限重试，可能产生重复提醒。稳定通知 ID 仅辅助去重，不能保证客户端不再次响铃。",
            ],
            "支持平台与资产": [
                "macOS ARM64：DMG 安装包与 SHA-256 校验文件、updater 压缩包与签名。",
                "Linux x86_64：API 服务包与 SHA-256 校验文件；latest.json 仅映射 darwin-aarch64，共七项资产。",
                "不提供 Linux 桌面包、网页版或安卓安装包。",
            ],
            "升级提示": [
                "先升级远程 API，再安装本机 App；分别发送一次推送测试，在设备上确认收到。Bark 已受理只表示服务端确认。",
                "沿用现有通知设置；自有 monitor 日志过滤目标应为 nexushub_webd。旧配置导入入口不再执行。",
                "安装前核对校验和及 updater 签名。macOS 包没有 Developer ID 签名或 Apple 公证，Minisign 不能替代系统代码签名。",
            ],
        }
        lines = [f"# NexusHub {version}", ""]
        for heading, paragraphs in sections.items():
            lines.extend([f"## {heading}", ""])
            lines.extend(f"- {paragraph}" for paragraph in paragraphs)
            lines.append("")
        return "\n".join(lines)

    if version == "1.2.14":
        sections = {
            "版本概览": ["本版下线 Gotify 通道，通知继续使用 Bark，并保留完整 AGENTS.md 折叠修复。"],
            "功能调整": [
                "移除 Gotify 设置卡片、测试按钮、发送器和本机及远程执行入口。",
                "保留 Codex、Claude Code、Grok 的完成、失败及需要回复通知，现有事件识别规则不变。",
            ],
            "问题修复": [
                "升级清理旧通道残留，避免已下线服务继续出现在设置和投递结果中。",
                "保留 AGENTS.md 结束标签与环境标签同行时的完整折叠，内部标题不再把正文拆到用户气泡。",
            ],
            "兼容性与迁移": [
                "自动清理 NexusHub 自有的旧通道配置、加密 Token、待投递记录及专属测试任务，并清理 SQLite 遗留页。",
                "保留 Bark 凭据、成功记录、去重状态和启用基线，不补发历史通知；未知投递结果不自动重发。",
                "API 协议保持 2，原有会话、Keychain 连接、管理员 API Key 与后台 monitor 继续使用。旧测试 RPC 不再执行。",
            ],
            "支持平台与资产": [
                "macOS ARM64：DMG 安装包与 SHA-256 校验文件、updater 压缩包与签名。",
                "Linux x86_64：API 服务包与 SHA-256 校验文件；latest.json 仅映射 darwin-aarch64，共七项资产。",
                "不提供 Linux 桌面包、网页版或安卓安装包。",
            ],
            "升级提示": [
                "先升级远程 API，再安装本机 App。单独部署的 Gotify 服务需由管理员卸载；NexusHub 不自动删除外部服务数据。",
                "安装前核对校验和及 updater 签名。macOS 包没有 Developer ID 签名或 Apple 公证；Minisign 不能替代系统代码签名。",
            ],
        }
        lines = [f"# NexusHub {version}", ""]
        for heading, paragraphs in sections.items():
            lines.extend([f"## {heading}", ""])
            lines.extend(f"- {paragraph}" for paragraph in paragraphs)
            lines.append("")
        return "\n".join(lines)

    if version == "1.2.12":
        sections = {
            "版本概览": ["本版精简子智能体与清理页面的说明文字，延续 1.2.11 的功能修复。"],
            "功能调整": [
                "子智能体汇总和列表移除统计不完整的通用说明，继续展示运行中、已完成、未知及不可用状态。",
                "归档清理、隐藏线程清理移除标题下方的重复说明，保留预览、确认、执行状态和逐项结果。",
            ],
            "问题修复": ["清理面板移除说明后，状态标记仍保持右对齐，避免留下多余的说明区域。"],
            "兼容性与迁移": [
                "API 协议保持 2，无数据库或配置迁移。既有父子关联校验、清理保护、跳过规则和真实错误反馈保持有效。",
                "继续支持子智能体开始与完成活动、任务名称和直属计数；内部页面包装默认隐藏。",
            ],
            "支持平台与资产": [
                "macOS ARM64：DMG 安装包与 SHA-256 校验文件、updater 压缩包与签名。",
                "Linux x86_64：API 服务压缩包与 SHA-256 校验文件；latest.json 仅映射 darwin-aarch64，共七项资产。",
            ],
            "升级提示": [
                "升级至 1.2.12 即可应用界面调整；现有会话、连接、API Key、Keychain 和通知设置无需重建。",
                "安装前核对校验和与 updater 签名。macOS 包没有 Developer ID 签名或 Apple 公证，现有资源封装的系统签名校验限制仍在；Minisign 签名用于验证更新归档来源。",
            ],
        }
        lines = [f"# NexusHub {version}", ""]
        for heading, paragraphs in sections.items():
            lines.extend([f"## {heading}", ""])
            lines.extend(f"- {paragraph}" for paragraph in paragraphs)
            lines.append("")
        return "\n".join(lines)

    if version == "1.2.11":
        sections = {
            "版本概览": ["本版修复 Codex 子智能体开始活动、任务名称和运行数量的展示，并隐藏会话中的内部页面标记。"],
            "功能调整": [
                "线程详情增加直属子智能体汇总，显示运行中、已完成及异常状态数量；主线程列表保持原样。",
                "点击汇总打开右侧列表，运行中的子智能体优先显示。可进入详情、查看其直属下级并逐级返回，保留阅读位置和焦点。",
                "活动行、列表与详情统一使用创建任务名；缺少任务名时再回退到已验证路径、线程标题或昵称。",
            ],
            "问题修复": [
                "识别原生“开始工作”“已完成”“已中断”和交互记录，保留发生顺序；完成后不再把历史开始记录改成完成记录。",
                "子智能体当前状态依据自己的回合判断，父线程没有新消息时仍刷新。按直属原生身份去重，避免重复回合或孙级任务影响数量。",
                "默认隐藏完整的 external_codex_apps_open_page 内部包装，复制、Plan 导出和搜索同步过滤；纯包装不再留下空消息或时间线短线，代码与引用示例保留。",
            ],
            "兼容性与迁移": [
                "远程 API 协议保持 2，沿用现有详情接口；不新增 RPC、数据库表或原生会话写入。旧服务缺少汇总时保留阅读并提示升级。",
                "关联缺失、冲突或读取不完整时显示未知或不可用，不推断完成。保留现有线程管理保护、机器隔离、Keychain、Bark 和 monitor。",
            ],
            "支持平台与资产": [
                "macOS ARM64：DMG 安装包与 SHA-256 校验文件、updater 压缩包与签名。",
                "Linux x86_64：nexushub-webd API 服务包与 SHA-256 校验文件，不提供网页或 Linux 桌面包。",
                "latest.json 仅映射 darwin-aarch64；共七项资产，darwin-arm64 文件名对应同一 macOS 架构。",
            ],
            "升级提示": [
                "先将远程 API 服务升级到 1.2.11，再安装本机 App，即可查看完整的直属汇总；现有连接、凭据、会话和通知设置无需重建。",
                "安装前核对校验和与 updater 签名。macOS 包尚无 Developer ID 签名或 Apple 公证；Minisign 验证更新归档来源，不能替代系统代码签名。",
                "覆盖安装若触发钥匙串授权，请在系统对话框完成验证。部署参数继续从仓库外提供。",
            ],
        }
        lines = [f"# NexusHub {version}", ""]
        for heading, paragraphs in sections.items():
            lines.extend([f"## {heading}", ""])
            lines.extend(f"- {paragraph}" for paragraph in paragraphs)
            lines.append("")
        return "\n".join(lines)

    if version == "1.2.10":
        sections = {
            "版本概览": ["本版修复 macOS App 远程连接可能持续加载的问题，延续 1.2.9 的紧凑阅读布局与 Codex 子智能体详情。"],
            "功能调整": [
                "钥匙串读取超过 15 秒会显示当前操作的错误，方便判断并重试；不会自动切换目标机器。",
                "保存和移除连接保持完整事务，等待提交或回滚完成后才允许切换机器。",
            ],
            "问题修复": [
                "钥匙串访问改为后台串行执行，避免多个远程请求同时读取凭据并阻塞异步运行时。",
                "超时或取消后，仍在执行的原生操作继续持有串行许可；尚未开始的任务可以取消，避免积压。",
                "读取凭据和验证服务能力后重新核对连接身份，防止旧连接请求继续执行或覆盖新机器结果。",
            ],
            "兼容性与迁移": [
                "远程 API 协议保持 2，不新增 RPC、数据库字段或凭据缓存。现有 API Key、Keychain 连接、会话、Bark 和监测配置继续使用。",
                "保留三个 Provider 的阅读、搜索、工具折叠、Plan 导出、附件和线程管理，以及 Codex 子智能体只读详情。",
            ],
            "支持平台与资产": [
                "macOS ARM64：DMG 安装包与 SHA-256 校验文件、updater 压缩包与签名。",
                "Linux x86_64：nexushub-webd API 服务包与 SHA-256 校验文件，不提供网页或 Linux 桌面包。",
                "latest.json 仅映射 darwin-aarch64；共七项资产，darwin-arm64 文件名对应同一 macOS 架构。",
            ],
            "升级提示": [
                "建议使用远程管理的用户升级至 1.2.10。覆盖安装后直接使用已有连接；本机 App 和远程服务分别更新。",
                "安装前核对校验和与 updater 签名。macOS 包尚无 Developer ID 签名或 Apple 公证；Minisign 验证更新归档来源，不能替代系统代码签名。",
                "若系统请求钥匙串授权，请在 macOS 对话框完成验证。连接失败时查看当前操作反馈，不需要删除原有凭据或会话。",
            ],
        }
        lines = [f"# NexusHub {version}", ""]
        for heading, paragraphs in sections.items():
            lines.extend([f"## {heading}", ""])
            lines.extend(f"- {paragraph}" for paragraph in paragraphs)
            lines.append("")
        return "\n".join(lines)

    if version == "1.2.9":
        sections = {
            "版本概览": ["本版让会话阅读更紧凑，并为 Codex 子智能体提供可点击的活动行与右侧详情。"],
            "功能调整": [
                "Codex、Claude Code 和 Grok 统一正文、工具摘要与操作位置；桌面复制按钮在悬停或键盘聚焦时出现，触屏保持可见。",
                "Codex 子智能体显示任务名称与自身回合状态，可查看委派内容、消息、Plan、工具和附件。支持嵌套返回及独立历史分页。",
                "宽窗口并排显示详情，窄窗口使用覆盖面板；关闭后恢复父线程位置和焦点，切换机器或主线程会关闭面板。",
            ],
            "问题修复": [
                "减少重复 Provider 标签、活动组间距和复制按钮造成的空白，保留原有阅读列宽度与字号。",
                "子智能体状态按其自身原生回合判断；关联缺失或有歧义时显示原因，不按名称打开其他线程。",
                "修复 Hook 集成测试对编译期二进制路径的硬依赖；测试继续启动本次 Cargo 构建的真实 CLI，验证确认等待和通知去重。",
            ],
            "兼容性与迁移": [
                "API 协议保持 2，新增子智能体读取能力。远程旧服务缺少能力时提示升级，现有本机读取不受影响。",
                "子智能体详情和附件均验证原主线程及父子关系；不新增数据库表，不改写原生会话，不开放执行或管理控制。",
                "保留搜索、用户指令时间线、工具折叠、AGENTS.md、Plan 导出、线程管理、Bark、Keychain 与机器隔离。",
            ],
            "支持平台与资产": [
                "macOS ARM64：DMG 安装包与 SHA-256 校验文件、updater 压缩包与签名。",
                "Linux x86_64：nexushub-webd API 服务包与 SHA-256 校验文件，不提供网页或 Linux 桌面包。",
                "latest.json 仅映射 darwin-aarch64；共七项资产，darwin-arm64 文件名对应同一 macOS 架构。",
            ],
            "升级提示": [
                "使用远程子智能体详情前，请先升级 API 服务，再安装 1.2.9 App。保留现有 API Key、Keychain 连接、会话及通知设置。",
                "安装前核对校验和与 updater 签名。macOS 包尚无 Developer ID 签名或 Apple 公证；Minisign 验证更新归档来源，不能替代系统代码签名。",
                "覆盖安装若触发钥匙串授权，请在系统对话框完成验证，无需重建远程连接。部署参数继续从仓库外提供。",
            ],
        }
        lines = [f"# NexusHub {version}", ""]
        for heading, paragraphs in sections.items():
            lines.extend([f"## {heading}", ""])
            lines.extend(f"- {paragraph}" for paragraph in paragraphs)
            lines.append("")
        return "\n".join(lines)

    if version == "1.2.8":
        sections = {
            "版本概览": ["本版改进线程管理与桌面启动体验，修复隐藏清理被单个受保护线程阻断的问题，并完成 Pi 集成退役。"],
            "功能调整": [
                "右键线程立即显示操作菜单；保留省略号、触摸和键盘入口，右键未选中线程不切换当前详情。",
                "Codex 普通与归档线程均可永久删除，支持最多 100 项批量预览与确认。",
                "隐藏清理只处理本次预览候选，逐项显示删除、跳过、失败和剩余结果。",
            ],
            "问题修复": [
                "修复 Grok 短标题、长标题及运行状态下的卡片背景宽度不一致。",
                "主窗口隐藏创建后一次填满当前屏幕工作区，避免启动时反复缩放；Dock 重开和机器切换保留窗口尺寸，不切换独立全屏 Space。",
                "受保护、文件已变化或单项失败的隐藏线程不再阻断其余候选；公共数据库、索引或存储错误会回滚整批。",
            ],
            "兼容性与迁移": [
                "远程 API 协议升级为 2。旧服务不能执行新版隐藏清理，App 会提示更新。",
                "移除 NexusHub 的 Pi 导航、读取、管理、通知与专属配置；仅清理 NexusHub 自有 Pi 记录，不卸载 Pi，不修改其原生配置、会话或工作目录。",
                "Codex、Claude Code 与 Grok 的搜索、附件、时间线、Plan 导出、Bark、Keychain 连接和机器隔离继续保留。",
            ],
            "支持平台与资产": [
                "macOS ARM64：DMG 安装包与 SHA-256 校验文件、updater 压缩包与签名。",
                "Linux x86_64：nexushub-webd API 服务包与 SHA-256 校验文件，不提供网页或 Linux 桌面包。",
                "latest.json 仅映射 darwin-aarch64；以上共七项资产，darwin-arm64 文件名对应同一 macOS 架构。",
            ],
            "升级提示": [
                "使用远程管理时先升级 API 服务，再安装 1.2.8 App；本机读取不依赖远程升级。",
                "安装前核对校验和与 updater 签名。永久删除不可撤销，请核对预览目标；运行中、状态未知或关联异常的线程仍受保护。",
                "部署主机、域名和凭据继续由仓库外提供。发布前检查源码、可达历史与资产；平台保留的不可达对象不属于可达历史审计覆盖范围。",
            ],
        }
        lines = [f"# NexusHub {version}", ""]
        for heading, paragraphs in sections.items():
            lines.extend([f"## {heading}", ""])
            lines.extend(f"- {paragraph}" for paragraph in paragraphs)
            lines.append("")
        return "\n".join(lines)

    version_parts = tuple(int(part) for part in version.split("."))
    has_remote_api = version_parts >= (1, 2, 0)
    has_linux_desktop = version_parts < (1, 1, 0)
    assets = [
        "macOS ARM64：DMG 安装包与 SHA-256 校验文件、updater 压缩包与签名。",
        "Linux x86_64：nexushub-webd 服务包与 SHA-256 校验文件。",
    ]
    if has_linux_desktop:
        assets.extend([
            "Linux x86_64 桌面：AppImage、DEB、RPM，附校验文件；AppImage 另附 updater 签名。",
            "latest.json 同时包含 macOS ARM64 与 Linux x86_64 桌面映射；macOS updater 另附 SHA-256 文件，本版共十五项资产。",
        ])
    else:
        assets.append("latest.json 仅映射 darwin-aarch64；本版共七项资产，不提供 Linux 桌面包。")
    if has_remote_api:
        migration = [
            "macOS App 支持本机与远程管理；远程连接使用独立管理员 API Key，保存在本机 Keychain。",
            "Linux 服务仅提供管理 API 与健康检查，不提供网页登录。",
        ]
        upgrade = [
            "升级前核对校验文件；远程地址与 API Key 在 App 中配置，部署参数由仓库外提供。",
            "保留会话、凭据和已有监测配置；连接提示协议不兼容时，先更新远程服务。",
        ]
    else:
        migration = [
            "本版支持 macOS App 与云端 WebUI；云端使用该版本的网页登录与认证设置。",
            "升级不改写原生会话；部署地址与认证参数从仓库外提供。",
        ]
        upgrade = [
            "本页描述历史版本的实际能力；管理员 API Key 远程管理从 1.2.0 开始提供。",
            "新安装建议使用当前稳定版。升级前核对校验文件，并阅读目标版本的迁移说明。",
        ]
    sections = {
        "版本概览": [f"NexusHub {version}，本版主要调整如下。"],
        "功能调整": highlights(version),
        "兼容性与迁移": migration,
        "支持平台与资产": assets,
        "升级提示": upgrade,
    }
    lines = [f"# NexusHub {version}", ""]
    for heading, paragraphs in sections.items():
        lines.extend([f"## {heading}", ""])
        lines.extend(f"- {paragraph}" for paragraph in paragraphs)
        lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("version", help="version without the leading v")
    parser.add_argument("--updater", action="store_true", help="render a short updater note")
    args = parser.parse_args()
    print(render(args.version, updater=args.updater), end="")
    return 0


if __name__ == "__main__":
    sys.exit(main())
