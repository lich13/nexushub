#!/usr/bin/env python3
"""Render concise Chinese release notes for GitHub Releases and updater metadata."""

from __future__ import annotations

import argparse
import sys


HIGHLIGHTS: dict[str, list[str]] = {
    "1.0.0": [
        "支持 Codex、Grok 和 Pi 的批量归档、恢复与删除。",
        "扩展 Bark 完成和失败通知，并移除已退役的日志维护入口。",
        "完成隐私清理和仓库结构整理，远程部署参数继续从仓库外提供。",
    ],
    "1.1.0": [
        "移除 NexusHub 自有 Goal 与自动恢复链路。",
        "Codex、Grok、Pi 统一显示运行状态，并将连续命令整理为可折叠执行组。",
        "发布资产收敛为 macOS App、updater 和 Linux webd 服务包。",
    ],
    "1.1.1": [
        "改进会话列表、归档线程操作和桌面交互稳定性。",
        "统一执行活动状态、键盘操作和移动端显示。",
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
        "改进远程管理连接、服务更新和桌面 App 更新体验。",
        "加强远程 API 的机器范围和更新状态隔离。",
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
        "增加普通 Codex 提问的 Bark 提醒、文件路径操作和统一设置页面。",
        "修复服务器更新在 systemd 只读隔离下无法写入的问题。",
    ],
    "1.2.6": [
        "修复本机与腾讯云切换后更新状态和能力缓存串机。",
        "统一本机 App 与腾讯云服务的更新反馈，保留远程 API Key 和监测配置。",
    ],
    "1.2.7": [
        "四个 Provider 的时间线短线仅显示识别到的用户指令，工具、计划和活动仍可通过原始锚点搜索。",
        "兼容常见 Claude Code 记录读取，未知或不完整记录继续保持只读。",
        "修复 Bark 完成通知在空结果或挂起工具时的状态判断。",
    ],
}


def highlights(version: str) -> list[str]:
    return HIGHLIGHTS.get(version, ["修复问题并改进稳定性。"])


def render(version: str, *, updater: bool = False) -> str:
    if updater:
        return f"NexusHub {version}：" + "；".join(highlights(version)[:2])

    lines = [f"# NexusHub {version}", "", "## 更新内容", ""]
    lines.extend(f"- {item}" for item in highlights(version))
    lines.extend(
        [
            "",
            "## 发布资产",
            "",
            "- macOS ARM64：DMG 安装包、校验文件、updater 压缩包和签名。",
            "- Linux x86_64：`nexushub-webd` 服务包及校验文件。",
            "- `latest.json` 仅提供 `darwin-aarch64` updater 映射；文件名中的 `darwin-arm64` 表示同一 macOS ARM64 架构。",
            "",
            "## 使用说明",
            "",
            "- macOS App 用于本机和远程 API 管理；远程地址与管理员 API Key 请在 App 中配置。",
            "- 服务端不提供网页登录，不在仓库或发布包中保存域名、主机、会话或凭据。",
            "- 升级前请核对校验文件，远程服务更新使用服务端 API。",
        ]
    )
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("version", help="version without the leading v")
    parser.add_argument("--updater", action="store_true", help="render a short updater note")
    args = parser.parse_args()
    print(render(args.version, updater=args.updater), end="")
    return 0


if __name__ == "__main__":
    sys.exit(main())
