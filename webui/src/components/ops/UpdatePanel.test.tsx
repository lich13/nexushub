import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test } from "vitest";
import { UpdatePanel } from "./UpdatePanel";
import { runtimeCapabilitiesForRuntime } from "../../lib/domain/capabilities";
import { demoUpdateStatus } from "../../lib/api/demo";
import type { ComponentProps } from "react";

function render(props: Partial<ComponentProps<typeof UpdatePanel>> = {}) {
  return renderToStaticMarkup(<UpdatePanel capabilities={runtimeCapabilitiesForRuntime("desktop")}
    loading={false} busy={false} finished={false} onAction={() => {}} {...props} />);
}
const status = { ...demoUpdateStatus("macos-tauri"), current_version: "1.2.2", latest_version: "v1.2.2", update_available: false };

describe("selected-machine update panel", () => {
  test("same or unknown versions hide install without claiming a successful check", () => {
    for (const value of [undefined, status, { ...status, latest_version: null, update_available: null }]) {
      const html = render({ status: value });
      expect(html).toContain("本机 App 更新");
      expect(html).toContain("检查更新");
      expect(html).not.toContain("更新至");
      expect(html).not.toContain("已是最新版本");
      expect(html).not.toContain("腾讯云服务更新");
    }
    expect(render({ status, action: "check", finished: true })).toContain("已是最新版本");
    expect(render({ status: { ...status, latest_version: "unknown" }, action: "check", finished: true })).not.toContain("已是最新版本");
  });
  test("only confirmed newer versions offer installation with normalized text", () => {
    const next = { ...status, latest_version: "v1.2.4", update_available: true };
    expect(render({ status: next })).toContain("更新至 1.2.4");
    expect(render({ status: { ...status, update_available: true } })).not.toContain("更新至");
    expect(render({ status: next, error: "读取失败" })).not.toContain("更新至");
    const failed = render({ status, action: "check", finished: true, error: "检查失败" });
    expect(failed).toContain('role="alert"');
    expect(failed).not.toContain("已是最新版本");
  });
  test("remote panel uses the same layout and preserves server backup cleanup", () => {
    const capabilities = { ...runtimeCapabilitiesForRuntime("desktop"), updateServiceLabels: true, updatePrune: true };
    const html = render({ capabilities, status });
    expect(html).toContain("腾讯云服务更新");
    expect(html).toContain("清理更新备份");
    expect(html).not.toContain("本机 App 更新");
    expect(html).not.toMatch(/Precheck|Latest|Current/);
  });
  test("active checks and installs disable actions and never show success", () => {
    const html = render({ status, busy: true, action: "install", finished: true });
    expect(html).toContain("正在下载并安装");
    expect(html).toContain("disabled");
    expect(html).not.toContain("已是最新版本");
    expect(html).not.toContain("更新完成");
  });
});
