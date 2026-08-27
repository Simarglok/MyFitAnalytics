import { mount, unmount } from "svelte";
import { describe, expect, it, vi } from "vitest";
import SourcesQualityPage from "./SourcesQualityPage.svelte";
import { MockTransport } from "../mock-transport";
import type { QualityItem } from "../types";

const qualityItem: QualityItem = {
  id: "quality-1",
  assetId: "asset-1",
  itemType: "import",
  severity: "warning",
  status: "failed",
  message: "One record needs review",
};

describe("SourcesQualityPage", () => {
  it("renders a quality issue and queues retry for its source asset", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const transport = new MockTransport({ qualityItems: [qualityItem] });
    const app = mount(SourcesQualityPage, { target, props: { transport } });

    await vi.waitFor(() =>
      expect(target.textContent).toContain(qualityItem.message),
    );
    const retry = [...target.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Retry import"),
    );
    expect(retry).toBeTruthy();
    retry?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    await vi.waitFor(() =>
      expect(transport.calls).toContain("retryAsset:asset-1"),
    );
    unmount(app);
  });
});
