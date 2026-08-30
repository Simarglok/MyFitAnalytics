import { mount, unmount } from "svelte";
import { describe, expect, it, vi } from "vitest";
import AppShell from "./AppShell.svelte";
import { MockTransport } from "../mock-transport";
import type {
  AttemptView,
  DashboardPageView,
  DataChangedEvent,
  DateRangeView,
  IngestionStatus,
  NavigationItemView,
  NavigationView,
  ScanTicket,
} from "../types";

const oldInitialRange: DateRangeView = {
  start: "2026-07-30",
  end: "2026-08-29",
};
const newInitialRange: DateRangeView = {
  start: "2026-01-04",
  end: "2026-02-03",
};

const overviewItem: NavigationItemView = {
  id: "base:overview",
  pageId: "overview",
  titleKey: "base.overview.title",
  moduleId: "base",
  availability: {
    state: "ready",
    reasonKey: "dashboard.ready",
    requiredCapabilities: [],
    requiredDependencies: [],
    action: null,
  },
};

function navigation(initialRange: DateRangeView): NavigationView {
  return { items: [overviewItem], initialRange };
}

function ingestionStatus(
  state: "attention" | "healthy",
  attentionItems: number,
): IngestionStatus {
  return {
    health: {
      state,
      workingJobs: 0,
      waitingAssets: 0,
      attentionItems,
      criticalItems: 0,
      failureCodeCounts:
        attentionItems > 0 ? { source_validation_failed: attentionItems } : {},
    },
    queueCapacity: 32,
    recoveryMode: "normal",
    configured: true,
    pendingModuleUpdates: [],
  };
}

class SequencedNavigationTransport extends MockTransport {
  private navigationIndex = 0;
  private dataChangedListener: ((event: DataChangedEvent) => void) | undefined;
  readonly dashboardRanges: DateRangeView[] = [];

  constructor(private readonly navigations: NavigationView[]) {
    super();
  }

  get hasDataChangedListener(): boolean {
    return this.dataChangedListener !== undefined;
  }

  override async getNavigation(): Promise<NavigationView> {
    return this.navigations[
      Math.min(this.navigationIndex, this.navigations.length - 1)
    ];
  }

  override async getDashboard(
    moduleId: string,
    pageId: string,
    range: DateRangeView,
  ): Promise<DashboardPageView> {
    this.dashboardRanges.push({ ...range });
    return super.getDashboard(moduleId, pageId, range);
  }

  override async refreshNow(): Promise<ScanTicket> {
    const ticket = await super.refreshNow();
    this.navigationIndex = Math.min(
      this.navigationIndex + 1,
      this.navigations.length - 1,
    );
    return ticket;
  }

  override async subscribeDataChanged(
    listener: (event: DataChangedEvent) => void,
  ): Promise<() => void> {
    this.dataChangedListener = listener;
    return () => {
      if (this.dataChangedListener === listener) {
        this.dataChangedListener = undefined;
      }
    };
  }

  advanceNavigation(): void {
    this.navigationIndex = Math.min(
      this.navigationIndex + 1,
      this.navigations.length - 1,
    );
  }

  emitDataChanged(): void {
    this.dataChangedListener?.({
      capabilities: ["body.weight"],
      dashboards: ["base:overview"],
    });
  }
}

class RetryHealthTransport extends MockTransport {
  private status = ingestionStatus("attention", 1);
  private dataChangedListener: ((event: DataChangedEvent) => void) | undefined;

  get hasDataChangedListener(): boolean {
    return this.dataChangedListener !== undefined;
  }

  override async getIngestionStatus(): Promise<IngestionStatus> {
    return this.status;
  }

  override async retryAsset(assetId: string): Promise<AttemptView> {
    this.calls.push(`retryAsset:${assetId}`);
    this.status = ingestionStatus("healthy", 0);
    this.dataChangedListener?.({ capabilities: [], dashboards: [] });
    return {
      assetId,
      attemptId: "attempt-1",
      status: "retry_queued",
      errorCode: null,
    };
  }

  override async subscribeDataChanged(
    listener: (event: DataChangedEvent) => void,
  ): Promise<() => void> {
    this.dataChangedListener = listener;
    return () => {
      if (this.dataChangedListener === listener) {
        this.dataChangedListener = undefined;
      }
    };
  }
}

async function expectVisibleRange(
  target: HTMLElement,
  range: DateRangeView,
): Promise<void> {
  await vi.waitFor(() => {
    expect(
      target.querySelector<HTMLInputElement>('[aria-label="Range start"]')
        ?.value,
    ).toBe(range.start);
    expect(
      target.querySelector<HTMLInputElement>('[aria-label="Range end"]')?.value,
    ).toBe(range.end);
  });
}

describe("AppShell dashboard range synchronization", () => {
  it("refreshes visible health after a successful retry notification", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const transport = new RetryHealthTransport();
    const app = mount(AppShell, { target, props: { transport } });

    await vi.waitFor(() =>
      expect(target.querySelector(".status-banner")?.textContent).toContain(
        "1 attention items",
      ),
    );
    await vi.waitFor(() => expect(transport.hasDataChangedListener).toBe(true));

    await transport.retryAsset("asset-1");

    await vi.waitFor(() => {
      expect(target.querySelector(".status-banner")?.textContent).toContain(
        "0 attention items",
      );
      expect(target.querySelector(".status-banner")?.className).toContain(
        "status-healthy",
      );
    });
    unmount(app);
  });

  it("rebases the dashboard range after Refresh when the backend initial range changes", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const transport = new SequencedNavigationTransport([
      navigation(oldInitialRange),
      navigation(newInitialRange),
    ]);
    const app = mount(AppShell, { target, props: { transport } });

    await expectVisibleRange(target, oldInitialRange);
    target.querySelector<HTMLButtonElement>(".shell-actions button")?.click();

    await expectVisibleRange(target, newInitialRange);
    expect(transport.dashboardRanges.at(-1)).toEqual(newInitialRange);
    unmount(app);
  });

  it("rebases the dashboard range after a data-change notification", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const transport = new SequencedNavigationTransport([
      navigation(oldInitialRange),
      navigation(newInitialRange),
    ]);
    const app = mount(AppShell, { target, props: { transport } });

    await expectVisibleRange(target, oldInitialRange);
    await vi.waitFor(() => expect(transport.hasDataChangedListener).toBe(true));
    transport.advanceNavigation();
    transport.emitDataChanged();

    await expectVisibleRange(target, newInitialRange);
    expect(transport.dashboardRanges.at(-1)).toEqual(newInitialRange);
    unmount(app);
  });

  it("preserves a custom dashboard range across a rebasing Refresh", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const transport = new SequencedNavigationTransport([
      navigation(oldInitialRange),
      navigation(newInitialRange),
    ]);
    const app = mount(AppShell, { target, props: { transport } });

    await expectVisibleRange(target, oldInitialRange);
    const start = target.querySelector<HTMLInputElement>(
      '[aria-label="Range start"]',
    );
    const end = target.querySelector<HTMLInputElement>(
      '[aria-label="Range end"]',
    );
    const form = target.querySelector<HTMLFormElement>(".range-controls");
    if (!start || !end || !form) {
      throw new Error("range controls were not rendered");
    }
    start.value = "2025-12-01";
    start.dispatchEvent(new Event("input", { bubbles: true }));
    end.value = "2025-12-31";
    end.dispatchEvent(new Event("input", { bubbles: true }));
    form.dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true }),
    );
    await vi.waitFor(() =>
      expect(transport.dashboardRanges.at(-1)).toEqual({
        start: "2025-12-01",
        end: "2025-12-31",
      }),
    );

    target.querySelector<HTMLButtonElement>(".shell-actions button")?.click();

    await expectVisibleRange(target, {
      start: "2025-12-01",
      end: "2025-12-31",
    });
    expect(transport.dashboardRanges.at(-1)).toEqual({
      start: "2025-12-01",
      end: "2025-12-31",
    });
    unmount(app);
  });
});
