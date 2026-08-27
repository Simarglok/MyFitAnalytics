// @vitest-environment node

import { dirname, join } from "node:path";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

function sourceFiles(root: string): string[] {
  return readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const path = join(root, entry.name);
    if (entry.isDirectory()) return sourceFiles(path);
    if (!/\.(ts|svelte)$/.test(entry.name) || entry.name.endsWith(".test.ts"))
      return [];
    return [path];
  });
}

describe("Tauri import boundary", () => {
  it("keeps @tauri-apps/api imports inside tauri-transport.ts", () => {
    const sourceRoot = dirname(fileURLToPath(import.meta.url));
    const violatingFiles = sourceFiles(sourceRoot).filter((path) => {
      if (path.endsWith(join("lib", "tauri-transport.ts"))) return false;
      return readFileSync(path, "utf8").includes("@tauri-apps/api");
    });
    expect(violatingFiles).toEqual([]);
    expect(
      readFileSync(join(sourceRoot, "lib", "tauri-transport.ts"), "utf8"),
    ).toContain("@tauri-apps/api");
  });
});
