import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import * as catalog from "./catalog";

function sourceFiles(root: string): string[] {
  return readdirSync(root, { recursive: true, withFileTypes: true })
    .filter(
      (entry) =>
        entry.isFile() &&
        (entry.name.endsWith(".svelte") || entry.name.endsWith(".ts")) &&
        !entry.name.endsWith(".test.ts"),
    )
    .map((entry) => join(entry.parentPath, entry.name));
}

describe("English UI catalog", () => {
  it("contains every literal message key used by the Svelte source", () => {
    const sourceRoot = join(process.cwd(), "src");
    const keys = new Set<string>();
    const expression = /message\(\s*["']([^"']+)["']/g;
    for (const path of sourceFiles(sourceRoot)) {
      const source = readFileSync(path, "utf8");
      for (const match of source.matchAll(expression)) keys.add(match[1]);
    }
    const missing = [...keys].filter(
      (key) => !catalog.catalogKeys().includes(key),
    );
    expect(missing).toEqual([]);
  });

  it("uses a visible marker instead of silently hiding a missing key", () => {
    expect(catalog.translate("dashboard.not_in_catalog")).toBe(
      "⟦dashboard.not_in_catalog⟧",
    );
    expect(catalog.translate("dashboard.not_in_catalog", "Fallback text")).toBe(
      "Fallback text",
    );
  });

  it("resolves selected locale, module English, then core English", () => {
    const resolveMessage = (
      catalog as typeof catalog & {
        resolveMessage?: (
          key: string,
          layers: Record<string, Record<string, string>>,
        ) => string;
      }
    ).resolveMessage;
    expect(resolveMessage).toBeTypeOf("function");
    expect(
      resolveMessage?.("module.metric", {
        locale: { "module.metric": "Métrique" },
        module: { "module.metric": "Metric" },
        core: {},
      }),
    ).toBe("Métrique");
    expect(
      resolveMessage?.("module.metric", {
        locale: {},
        module: { "module.metric": "Metric" },
        core: {},
      }),
    ).toBe("Metric");
    expect(
      resolveMessage?.("dashboard.coverage", {
        locale: {},
        module: {},
        core: catalog.ENGLISH_CATALOG,
      }),
    ).toBe("Coverage");
  });
});
