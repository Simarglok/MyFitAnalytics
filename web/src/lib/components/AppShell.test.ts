import { mount, unmount } from "svelte";
import { describe, expect, it, vi } from "vitest";
import AppShell from "./AppShell.svelte";
import { MockTransport } from "../mock-transport";
import type {
  AttemptView,
  BootstrapState,
  DashboardPageView,
  DataChangedEvent,
  DateRangeView,
  IngestionStatus,
  ModuleCatalogEntry,
  ModuleView,
  NavigationItemView,
  NavigationView,
  ProviderView,
  ScanTicket,
  WorkspaceView,
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

const settingsWorkspace: WorkspaceView = {
  workspaceRoot: "/tmp/settings-workspace",
  appDataRoot: "/tmp/settings-app-data",
  databasePath: "/tmp/settings-app-data/myfitanalytics.duckdb",
  recoveryPath: "/tmp/settings-app-data/recovery",
  backupPath: "/tmp/settings-app-data/recovery",
  archiveRoot: "/tmp/settings-workspace/archive",
  sourcePaths: [],
};

const settingsModules: ModuleView[] = [
  {
    id: "base",
    moduleType: "dashboard",
    version: "1.0.0",
    enabled: true,
    localizationNamespace: "dashboard.base",
    displayName: "base",
  },
  {
    id: "mynetdiary",
    moduleType: "source",
    version: "1.0.0",
    enabled: true,
    localizationNamespace: "source.mynetdiary",
    displayName: "MyNetDiary",
    providedCapabilities: ["activity.days"],
  },
];

function settingsCatalog(updated: Set<string>): ModuleCatalogEntry[] {
  return settingsModules.map((module) => ({
    module: { ...module },
    origin: module.id === "base" ? "installed" : "bundled",
    installState: updated.has(module.id) ? "enabled" : "update",
    availableVersion: updated.has(module.id) ? null : "2.0.0",
    errorCode: null,
  }));
}

function settingsNavigation(providerSelected: boolean): NavigationView {
  return {
    items: [
      {
        id: "base:overview",
        pageId: "overview",
        titleKey: "base.overview.title",
        moduleId: "base",
        availability: {
          state: providerSelected ? "waiting_for_data" : "missing_capability",
          reasonKey: providerSelected
            ? "dashboard.waiting_for_data"
            : "dashboard.missing_capability",
          requiredCapabilities: ["activity.days"],
          requiredDependencies: [],
          action: providerSelected
            ? "dashboard.action.import_data"
            : "dashboard.action.configure_source",
        },
      },
    ],
    initialRange: oldInitialRange,
  };
}

function settingsStatus(modulesUpdated: boolean): IngestionStatus {
  return {
    ...ingestionStatus("healthy", 0),
    configured: modulesUpdated,
    pendingModuleUpdates: modulesUpdated ? [] : ["MyNetDiary", "base"],
  };
}

class SettingsMutationTransport extends MockTransport {
  private readonly updated = new Set<string>();
  private providerSelected = false;

  constructor() {
    super();
  }

  override async getBootstrapState(): Promise<BootstrapState> {
    return {
      productName: "MyFitAnalytics",
      locale: "en-US",
      activeProviders: this.providerSelected
        ? { "activity.days": "mynetdiary" }
        : {},
      modules: settingsModules.map((module) => ({ ...module })),
    };
  }

  override async getNavigation(): Promise<NavigationView> {
    return settingsNavigation(this.providerSelected);
  }

  override async getIngestionStatus(): Promise<IngestionStatus> {
    return settingsStatus(this.updated.size === settingsModules.length);
  }

  override async listModuleCatalog(): Promise<ModuleCatalogEntry[]> {
    return settingsCatalog(this.updated);
  }

  override async getWorkspaceView(): Promise<WorkspaceView> {
    return settingsWorkspace;
  }

  override async updateModule(moduleId: string): Promise<ModuleView> {
    this.calls.push(`updateModule:${moduleId}`);
    this.updated.add(moduleId);
    const module = settingsModules.find(
      (candidate) => candidate.id === moduleId,
    );
    if (!module) throw new Error(`unknown module ${moduleId}`);
    return { ...module };
  }

  override async selectProvider(
    capability: string,
    moduleId: string,
  ): Promise<ProviderView> {
    this.calls.push(`selectProvider:${capability}:${moduleId}`);
    this.providerSelected = true;
    return {
      capability,
      moduleId,
      activeProviders: { [capability]: moduleId },
    };
  }
}

interface DeferredValue<T> {
  promise: Promise<T>;
  resolve: (value: T) => void;
}

function deferredValue<T>(): DeferredValue<T> {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolver) => {
    resolve = resolver;
  });
  return { promise, resolve };
}

interface ShellSnapshot {
  bootstrap: DeferredValue<BootstrapState>;
  navigation: DeferredValue<NavigationView>;
  ingestionStatus: DeferredValue<IngestionStatus>;
  resolve(): void;
}

function shellSnapshot(
  productName: string,
  providerSelected: boolean,
  pendingModuleUpdates: string[],
): ShellSnapshot {
  const bootstrap = deferredValue<BootstrapState>();
  const navigation = deferredValue<NavigationView>();
  const ingestionStatus = deferredValue<IngestionStatus>();
  const bootstrapState: BootstrapState = {
    productName,
    locale: "en-US",
    activeProviders: providerSelected ? { "activity.days": "mynetdiary" } : {},
    modules: settingsModules.map((module) => ({ ...module })),
  };
  const navigationView: NavigationView = settingsNavigation(providerSelected);
  const ingestionStatusView: IngestionStatus = {
    ...settingsStatus(pendingModuleUpdates.length === 0),
    pendingModuleUpdates,
  };
  return {
    bootstrap,
    navigation,
    ingestionStatus,
    resolve: () => {
      bootstrap.resolve(bootstrapState);
      navigation.resolve(navigationView);
      ingestionStatus.resolve(ingestionStatusView);
    },
  };
}

class OutOfOrderShellTransport extends MockTransport {
  private readonly snapshots: ShellSnapshot[] = [
    shellSnapshot("Initial shell", false, ["legacy-module"]),
    shellSnapshot("Older shell", false, ["stale-module"]),
    shellSnapshot("Latest shell", true, []),
  ];
  private snapshotIndex = 0;
  private currentSnapshot: ShellSnapshot | null = null;
  private dataChangedListener: ((event: DataChangedEvent) => void) | undefined;

  constructor() {
    super();
    this.snapshots[0].resolve();
  }

  get hasDataChangedListener(): boolean {
    return this.dataChangedListener !== undefined;
  }

  override getBootstrapState(): Promise<BootstrapState> {
    this.currentSnapshot =
      this.snapshots[Math.min(this.snapshotIndex, this.snapshots.length - 1)];
    this.snapshotIndex += 1;
    return this.currentSnapshot.bootstrap.promise;
  }

  override getNavigation(): Promise<NavigationView> {
    if (!this.currentSnapshot) throw new Error("bootstrap was not requested");
    return this.currentSnapshot.navigation.promise;
  }

  override getIngestionStatus(): Promise<IngestionStatus> {
    if (!this.currentSnapshot) throw new Error("bootstrap was not requested");
    return this.currentSnapshot.ingestionStatus.promise;
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

  async resolveSnapshot(index: number): Promise<void> {
    const snapshot = this.snapshots[index];
    if (!snapshot) return;
    snapshot.resolve();
    await Promise.all([
      snapshot.bootstrap.promise,
      snapshot.navigation.promise,
      snapshot.ingestionStatus.promise,
    ]);
    await Promise.resolve();
  }

  emitDataChanged(): void {
    this.dataChangedListener?.({ capabilities: [], dashboards: [] });
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

  it("refreshes shell state after Settings updates and provider selection", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const transport = new SettingsMutationTransport();
    const app = mount(AppShell, { target, props: { transport } });

    await vi.waitFor(() =>
      expect(
        target.querySelector('[data-action="open-settings"]'),
      ).toBeTruthy(),
    );
    target
      .querySelector<HTMLButtonElement>('[data-action="open-settings"]')
      ?.click();

    await vi.waitFor(() =>
      expect(
        target.querySelector(
          '[data-action="update"][data-module-id="mynetdiary"]',
        ),
      ).toBeTruthy(),
    );
    target
      .querySelector<HTMLButtonElement>(
        '[data-action="update"][data-module-id="mynetdiary"]',
      )
      ?.click();
    await vi.waitFor(() =>
      expect(transport.calls).toContain("updateModule:mynetdiary"),
    );
    await vi.waitFor(() =>
      expect(
        target.querySelector('[data-action="update"][data-module-id="base"]'),
      ).toBeTruthy(),
    );
    target
      .querySelector<HTMLButtonElement>(
        '[data-action="update"][data-module-id="base"]',
      )
      ?.click();
    await vi.waitFor(() =>
      expect(transport.calls).toContain("updateModule:base"),
    );

    await vi.waitFor(() => {
      expect(target.querySelector(".status-banner")?.textContent).not.toContain(
        "Module updates required",
      );
      expect(target.querySelector(".status-banner")?.textContent).not.toContain(
        "Not configured",
      );
      expect(
        target
          .querySelector("[data-availability-state]")
          ?.getAttribute("data-availability-state"),
      ).toBe("missing_capability");
      expect(
        [...target.querySelectorAll("button")].find(
          (button) => button.getAttribute("aria-current") === "page",
        )?.textContent,
      ).toContain("Settings");
    });

    target
      .querySelector<HTMLButtonElement>(
        '[data-action="provider"][data-module-id="mynetdiary"]',
      )
      ?.click();
    await vi.waitFor(() =>
      expect(transport.calls).toContain(
        "selectProvider:activity.days:mynetdiary",
      ),
    );
    await vi.waitFor(() => {
      expect(
        target
          .querySelector("[data-availability-state]")
          ?.getAttribute("data-availability-state"),
      ).toBe("waiting_for_data");
      expect(
        target.querySelector('[data-module-id="mynetdiary"]')?.textContent,
      ).toContain("Active provider");
      expect(
        [...target.querySelectorAll("button")].find(
          (button) => button.getAttribute("aria-current") === "page",
        )?.textContent,
      ).toContain("Settings");
    });
    unmount(app);
  });

  it("keeps the newest shell snapshot when reload responses finish out of order", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const transport = new OutOfOrderShellTransport();
    const app = mount(AppShell, { target, props: { transport } });

    await vi.waitFor(() =>
      expect(target.querySelector(".masthead h1")?.textContent).toContain(
        "Initial shell",
      ),
    );
    await vi.waitFor(() => expect(transport.hasDataChangedListener).toBe(true));

    transport.emitDataChanged();
    transport.emitDataChanged();
    await transport.resolveSnapshot(2);

    await vi.waitFor(() => {
      expect(target.querySelector(".masthead h1")?.textContent).toContain(
        "Latest shell",
      );
      expect(target.querySelector(".status-banner")?.textContent).not.toContain(
        "Module updates required",
      );
      expect(
        target
          .querySelector("[data-availability-state]")
          ?.getAttribute("data-availability-state"),
      ).toBe("waiting_for_data");
    });

    await transport.resolveSnapshot(1);
    await vi.waitFor(() =>
      expect(target.querySelector(".masthead h1")?.textContent).toContain(
        "Latest shell",
      ),
    );
    expect(target.querySelector(".status-banner")?.textContent).not.toContain(
      "stale-module",
    );
    unmount(app);
  });
});
