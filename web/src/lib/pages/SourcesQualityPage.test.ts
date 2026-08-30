import { mount, unmount } from "svelte";
import { describe, expect, it, vi } from "vitest";
import SourcesQualityPage from "./SourcesQualityPage.svelte";
import { MockTransport } from "../mock-transport";
import type { AttemptView, QualityItem } from "../types";

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

class RejectedRetryTransport extends MockTransport {
  override async retryAsset(assetId: string): Promise<AttemptView> {
    this.calls.push(`retryAsset:${assetId}`);
    throw {
      code: "retry_failed",
      message: "the invalid fixture still has no required sheet",
    };
  }
}

class RecoveringRetryTransport extends MockTransport {
  retryAttempts = 0;
  listQualityCalls = 0;

  override async listQualityItems(): Promise<QualityItem[]> {
    this.listQualityCalls += 1;
    return this.retryAttempts >= 2 ? [] : [qualityItem];
  }

  override async retryAsset(assetId: string): Promise<AttemptView> {
    this.retryAttempts += 1;
    this.calls.push(`retryAsset:${assetId}`);
    if (this.retryAttempts === 1) {
      throw {
        code: "retry_failed",
        message: "the invalid fixture still has no required sheet",
      };
    }
    return {
      assetId,
      attemptId: "attempt-2",
      status: "retry_queued",
      errorCode: null,
    };
  }
}

class ReloadFailureTransport extends MockTransport {
  listQualityCalls = 0;

  override async listQualityItems(): Promise<QualityItem[]> {
    this.listQualityCalls += 1;
    if (this.listQualityCalls === 1) return [qualityItem];
    throw {
      code: "quality_list_failed",
      message: "quality list temporarily unavailable",
    };
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

  it("preserves the last-good failure row and Retry action when retry rejects", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const transport = new RejectedRetryTransport({
      qualityItems: [qualityItem],
    });
    const app = mount(SourcesQualityPage, { target, props: { transport } });

    await vi.waitFor(() =>
      expect(target.querySelector("tbody tr")?.textContent).toContain(
        qualityItem.message,
      ),
    );
    target
      .querySelector<HTMLButtonElement>("tbody button")
      ?.dispatchEvent(new MouseEvent("click", { bubbles: true }));

    await vi.waitFor(() =>
      expect(transport.calls).toContain("retryAsset:asset-1"),
    );
    await vi.waitFor(() => {
      expect(target.querySelectorAll("tbody tr")).toHaveLength(1);
      expect(target.querySelector("tbody tr")?.textContent).toContain(
        qualityItem.message,
      );
      expect(
        [...target.querySelectorAll("tbody button")].some((button) =>
          button.textContent?.includes("Retry import"),
        ),
      ).toBe(true);
      expect(target.querySelector("[data-retry-error]")?.textContent).toContain(
        "retry_failed",
      );
      expect(target.querySelector("[data-list-error]")).toBeNull();
    });
    unmount(app);
  });

  it("recovers the quality list after a later retry succeeds", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const transport = new RecoveringRetryTransport();
    const app = mount(SourcesQualityPage, { target, props: { transport } });

    await vi.waitFor(() =>
      expect(target.querySelector("tbody tr")?.textContent).toContain(
        qualityItem.message,
      ),
    );
    target.querySelector<HTMLButtonElement>("tbody button")?.click();
    await vi.waitFor(() =>
      expect(target.querySelector("[data-retry-error]")).toBeTruthy(),
    );
    target.querySelector<HTMLButtonElement>("tbody button")?.click();

    await vi.waitFor(() =>
      expect(transport.calls).toEqual([
        "retryAsset:asset-1",
        "retryAsset:asset-1",
      ]),
    );
    await vi.waitFor(() => {
      expect(transport.listQualityCalls).toBe(2);
      expect(target.textContent).toContain("No quality issues found.");
      expect(target.querySelector("tbody")).toBeNull();
      expect(target.querySelector("[data-retry-error]")).toBeNull();
      expect(target.querySelector("[data-list-error]")).toBeNull();
    });
    unmount(app);
  });

  it("keeps list-load errors distinct when the initial quality load fails", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const app = mount(SourcesQualityPage, {
      target,
      props: {
        transport: new MockTransport({
          error: {
            code: "quality_list_failed",
            message: "quality list unavailable",
          },
        }),
      },
    });

    await vi.waitFor(() =>
      expect(target.querySelector("[data-list-error]")?.textContent).toContain(
        "quality_list_failed",
      ),
    );
    expect(target.querySelector("tbody")).toBeNull();
    expect(target.querySelector("[data-retry-error]")).toBeNull();
    unmount(app);
  });

  it("preserves last-good rows when a later quality list load fails", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const transport = new ReloadFailureTransport();
    const app = mount(SourcesQualityPage, { target, props: { transport } });

    await vi.waitFor(() =>
      expect(target.querySelector("tbody tr")?.textContent).toContain(
        qualityItem.message,
      ),
    );
    target.querySelector<HTMLButtonElement>(".page-heading button")?.click();

    await vi.waitFor(() =>
      expect(target.querySelector("[data-list-error]")?.textContent).toContain(
        "quality_list_failed",
      ),
    );
    expect(target.querySelectorAll("tbody tr")).toHaveLength(1);
    expect(
      [...target.querySelectorAll("tbody button")].some((button) =>
        button.textContent?.includes("Retry import"),
      ),
    ).toBe(true);
    expect(target.querySelector("[data-retry-error]")).toBeNull();
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
