#!/usr/bin/env python3
"""Render concise Chinese release notes for GitHub Releases and updater metadata."""

from __future__ import annotations

import argparse
import sys


HIGHLIGHTS: dict[str, list[str]] = {
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
