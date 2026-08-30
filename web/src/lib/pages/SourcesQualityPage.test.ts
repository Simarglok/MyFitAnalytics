import { mount, unmount } from "svelte";
import { describe, expect, it, vi } from "vitest";
import SourcesQualityPage from "./SourcesQualityPage.svelte";
import { MockTransport } from "../mock-transport";
import type { QualityItem } from "../types";

const qualityItem: QualityItem = {
  id: "quality-1",
  assetId: "asset-1",
  code: null,
  itemType: "import",
  severity: "warning",
  status: "failed",
  message: "One record needs review",
};

const currentFailure: QualityItem = {
  id: "asset:asset-2",
  assetId: null,
  code: "source_validation_failed",
  itemType: "import",
  severity: "error",
  status: "failed",
  message: "source_validation_failed",
};

class CountingTransport extends MockTransport {
  listQualityCalls = 0;

  override async listQualityItems(): Promise<QualityItem[]> {
    this.listQualityCalls += 1;
    return super.listQualityItems();
  }
}

describe("SourcesQualityPage", () => {
  it("renders a quality issue and queues retry for its source asset", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const transport = new CountingTransport({ qualityItems: [qualityItem] });
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
    await vi.waitFor(() => expect(transport.listQualityCalls).toBe(2));
    unmount(app);
  });

  it("renders a non-asset current failure without offering retry", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const app = mount(SourcesQualityPage, {
      target,
      props: {
        transport: new MockTransport({ qualityItems: [currentFailure] }),
      },
    });

    await vi.waitFor(() =>
      expect(target.querySelector("[data-quality-code]")?.textContent).toBe(
        currentFailure.code,
      ),
    );
    expect(
      [...target.querySelectorAll("button")].some((button) =>
        button.textContent?.includes("Retry import"),
      ),
    ).toBe(false);
    unmount(app);
  });

  it("keeps Settings controls out of the quality page", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const app = mount(SourcesQualityPage, {
      target,
      props: { transport: new MockTransport() },
    });

    await vi.waitFor(() =>
      expect(target.textContent).toContain("Data quality"),
    );
    expect(target.querySelector("#settings-title")).toBeNull();
    expect(target.querySelector('[data-action="choose-workspace"]')).toBeNull();
    unmount(app);
  });
});
