import { describe, expect, test } from "vitest";
import type { SessionStorageSize } from "../../types";
import { formatSessionStorage } from "./sessionStorage";

function storage(overrides: Partial<SessionStorageSize> = {}): SessionStorageSize {
  return {
    bytes: 1024,
    scope: "file",
    status: "complete",
    ...overrides
  };
}

describe("formatSessionStorage", () => {
  test.each([
    [undefined, "missing size"],
    [null, "null size"],
    [storage({ bytes: null, status: "unavailable" }), "unavailable size"],
    [storage({ bytes: Number.NaN }), "NaN size"],
    [storage({ bytes: Number.POSITIVE_INFINITY }), "infinite size"],
    [storage({ bytes: -1 }), "negative size"]
  ])("uses a muted unknown value for %s", (value) => {
    expect(formatSessionStorage(value)).toEqual({
      label: "—",
      tone: "muted",
      title: "会话大小暂不可用"
    });
  });

  test.each([
    [storage({ scope: "file", bytes: 0 }), { label: "0K", tone: "success", title: "会话文件 · 0 字节" }],
    [storage({ scope: "file", bytes: 1023 }), { label: "<1K", tone: "success", title: "会话文件 · 1,023 字节" }],
    [storage({ scope: "file", bytes: 1024 }), { label: "1K", tone: "success", title: "会话文件 · 1,024 字节" }],
    [storage({ scope: "file", bytes: 1536 }), { label: "1.5K", tone: "success", title: "会话文件 · 1,536 字节" }],
    [storage({ scope: "directory", bytes: 1024 ** 2 }), { label: "1M", tone: "success", title: "会话目录 · 1,048,576 字节" }],
    [storage({ scope: "directory", bytes: Math.floor(1024 ** 2 * 1.29) }), { label: "1.2M", tone: "success", title: "会话目录 · 1,352,663 字节" }],
    [storage({ scope: "directory", bytes: 1024 ** 3 }), { label: "1G", tone: "danger", title: "会话目录 · 1,073,741,824 字节" }]
  ])("formats complete %s values with a byte-based tone", (value, expected) => {
    expect(formatSessionStorage(value)).toEqual(expected);
  });

  for (const status of ["complete", "partial"] as const) {
    test.each([
      [100 * 1024 ** 2 - 1, "99.9M", "success"],
      [100 * 1024 ** 2, "100M", "warning"],
      [100 * 1024 ** 2 + 1, "100M", "warning"],
      [300 * 1024 ** 2 - 1, "299.9M", "warning"],
      [300 * 1024 ** 2, "300M", "danger"],
      [300 * 1024 ** 2 + 1, "300M", "danger"]
    ] as const)(`${status} size %s uses exact bytes at the 100M and 300M boundaries`, (bytes, label, tone) => {
      const partial = status === "partial";
      expect(formatSessionStorage(storage({ bytes, status, scope: "directory" }))).toEqual({
        label: `${partial ? "≥" : ""}${label}`,
        tone,
        title: `会话目录 · ${partial ? "至少 " : ""}${bytes.toLocaleString("en-US")} 字节${partial ? "（统计不完整）" : ""}`
      });
    });
  }

  test("removes a trailing .0 while retaining one decimal place", () => {
    expect(formatSessionStorage(storage({ bytes: 2048 }))).toMatchObject({ label: "2K" });
    expect(formatSessionStorage(storage({ bytes: 2200 }))).toMatchObject({ label: "2.1K" });
  });

  test("marks a partial directory size as a lower bound and explains the incomplete scan", () => {
    expect(formatSessionStorage(storage({
      scope: "directory",
      bytes: 1536,
      status: "partial"
    }))).toEqual({
      label: "≥1.5K",
      tone: "success",
      title: "会话目录 · 至少 1,536 字节（统计不完整）"
    });
  });

  test("keeps the partial marker for a sub-Kilobyte lower bound", () => {
    expect(formatSessionStorage(storage({ bytes: 1, status: "partial" }))).toEqual({
      label: "≥0K",
      tone: "success",
      title: "会话文件 · 至少 1 字节（统计不完整）"
    });
  });

  test.each([
    [0, "≥0K", "success"],
    [1024 ** 2, "≥1M", "success"],
    [1024 ** 3, "≥1G", "danger"]
  ] as const)("retains the lower-bound marker and byte-based tone for partial %s", (bytes, label, tone) => {
    expect(formatSessionStorage(storage({ bytes, status: "partial" }))).toMatchObject({ label, tone });
  });

  test("announces pending directory work without presenting an old byte count", () => {
    expect(formatSessionStorage(storage({
      scope: "directory",
      bytes: 1024,
      status: "pending"
    }))).toEqual({
      label: "—",
      tone: "muted",
      title: "会话目录 · 正在统计"
    });
  });
});
