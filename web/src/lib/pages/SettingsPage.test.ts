import { mount, unmount } from "svelte";
import { describe, expect, it, vi } from "vitest";
import SettingsPage from "./SettingsPage.svelte";
import { MockTransport } from "../mock-transport";
import type {
  BootstrapState,
  ModuleCatalogEntry,
  ProviderView,
  WorkspaceView,
} from "../types";
import type { MockTransportOptions } from "../mock-transport";

const workspace: WorkspaceView = {
  workspaceRoot: "/tmp/health-workspace",
  appDataRoot: "/tmp/app-data",
  databasePath: "/tmp/app-data/myfitanalytics.duckdb",
  recoveryPath: "/tmp/app-data/recovery",
  backupPath: "/tmp/app-data/recovery",
  archiveRoot: "/tmp/health-workspace/archive",
  sourcePaths: [
    {
      moduleId: "hevy",
      inboxPath: "/tmp/health-workspace/inbox/hevy",
      archivePath: "/tmp/health-workspace/archive/hevy",
    },
  ],
};

const catalog: ModuleCatalogEntry[] = [
  {
    module: {
      id: "hevy",
      moduleType: "source",
      version: "1.0.0",
      enabled: true,
      localizationNamespace: "source.hevy",
      providedCapabilities: ["body.weight", "strength.sets"],
    },
    origin: "bundled",
    installState: "enabled",
    availableVersion: null,
    errorCode: null,
  },
  {
    module: {
      id: "mynetdiary",
      moduleType: "source",
      version: "0.0.0",
      enabled: false,
      localizationNamespace: "source.mynetdiary",
      providedCapabilities: ["nutrition.items"],
    },
    origin: "bundled",
    installState: "available",
    availableVersion: "1.0.0",
    errorCode: null,
  },
  {
    module: {
      id: "broken-source",
      moduleType: "source",
      version: "0.0.0",
      enabled: false,
      localizationNamespace: "source.broken",
      providedCapabilities: [],
    },
    origin: "installed",
    installState: "incompatible",
    availableVersion: null,
    errorCode: "incompatible_source_api",
  },
  {
    module: {
      id: "summary-dashboard",
      moduleType: "dashboard",
      version: "1.0.0",
      enabled: false,
      localizationNamespace: "dashboard.summary",
    },
    origin: "installed",
    installState: "disabled",
    availableVersion: null,
    errorCode: null,
  },
  {
    module: {
      id: "en-locale",
      moduleType: "locale",
      version: "1.0.0",
      enabled: false,
      localizationNamespace: "locale.en",
    },
    origin: "installed",
    installState: "disabled",
    availableVersion: null,
    errorCode: null,
  },
];

class PersistedProviderTransport extends MockTransport {
  private activeProviders: Record<string, string>;

  constructor(options: MockTransportOptions = {}) {
    super(options);
    this.activeProviders = { ...(options.bootstrap?.activeProviders ?? {}) };
  }

  override async getBootstrapState(): Promise<BootstrapState> {
    const bootstrap = await super.getBootstrapState();
    return {
      ...bootstrap,
      activeProviders: { ...this.activeProviders },
    };
  }

  override async selectProvider(
    capability: string,
    moduleId: string,
  ): Promise<ProviderView> {
    const selection = await super.selectProvider(capability, moduleId);
    this.activeProviders = {
      ...this.activeProviders,
      ...selection.activeProviders,
    };
    return selection;
  }
}

class BootstrapErrorTransport extends PersistedProviderTransport {
  override async getBootstrapState(): Promise<BootstrapState> {
    throw {
      code: "bootstrap_failed",
      message: "raw bootstrap failure detail",
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

interface SettingsSnapshot {
  bootstrap: DeferredValue<BootstrapState>;
  catalog: DeferredValue<ModuleCatalogEntry[]>;
  workspace: DeferredValue<WorkspaceView>;
  resolve(): void;
}

function labeledCatalog(label: string): ModuleCatalogEntry[] {
  return catalog.map((entry, index) =>
    index === 0
      ? { ...entry, module: { ...entry.module, displayName: label } }
      : { ...entry, module: { ...entry.module } },
  );
}

function settingsSnapshot(label: string): SettingsSnapshot {
  const bootstrap = deferredValue<BootstrapState>();
  const catalogValue = deferredValue<ModuleCatalogEntry[]>();
  const workspaceValue = deferredValue<WorkspaceView>();
  const bootstrapState: BootstrapState = {
    productName: "MyFitAnalytics",
    locale: "en-US",
    activeProviders: {},
    modules: [],
  };
  const workspaceState: WorkspaceView = {
    ...workspace,
    workspaceRoot: `/tmp/${label.toLowerCase()}-workspace`,
  };
  return {
    bootstrap,
    catalog: catalogValue,
    workspace: workspaceValue,
    resolve: () => {
      bootstrap.resolve(bootstrapState);
      catalogValue.resolve(labeledCatalog(label));
      workspaceValue.resolve(workspaceState);
    },
  };
}

class OutOfOrderSettingsTransport extends PersistedProviderTransport {
  private readonly snapshots = [
    settingsSnapshot("Older module"),
    settingsSnapshot("Newest module"),
  ];
  private snapshotIndex = 0;
  private currentSnapshot: SettingsSnapshot | null = null;

  constructor() {
    super({ catalog, workspace });
  }

  get reloadCount(): number {
    return this.snapshotIndex;
  }

  override getBootstrapState(): Promise<BootstrapState> {
    this.currentSnapshot =
      this.snapshots[Math.min(this.snapshotIndex, this.snapshots.length - 1)];
    this.snapshotIndex += 1;
    return this.currentSnapshot.bootstrap.promise;
  }

  override listModuleCatalog(): Promise<ModuleCatalogEntry[]> {
    if (!this.currentSnapshot) throw new Error("bootstrap was not requested");
    return this.currentSnapshot.catalog.promise;
  }

  override getWorkspaceView(): Promise<WorkspaceView> {
    if (!this.currentSnapshot) throw new Error("bootstrap was not requested");
    return this.currentSnapshot.workspace.promise;
  }

  override async chooseWorkspaceRoot(): Promise<WorkspaceView> {
    return workspace;
  }

  async resolveSnapshot(index: number): Promise<void> {
    const snapshot = this.snapshots[index];
    if (!snapshot) return;
    snapshot.resolve();
    await Promise.all([
      snapshot.bootstrap.promise,
      snapshot.catalog.promise,
      snapshot.workspace.promise,
    ]);
    await Promise.resolve();
  }
}

describe("SettingsPage", () => {
  it("keeps the newest catalog when Settings reloads finish out of order", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const transport = new OutOfOrderSettingsTransport();
    const app = mount(SettingsPage, { target, props: { transport } });

    await vi.waitFor(() =>
      expect(
        target.querySelector('[data-action="choose-workspace"]'),
      ).toBeTruthy(),
    );
    target
      .querySelector<HTMLButtonElement>('[data-action="choose-workspace"]')
      ?.click();
    await vi.waitFor(() => expect(transport.reloadCount).toBe(2));

    await transport.resolveSnapshot(1);
    await vi.waitFor(() =>
      expect(
        target.querySelector('[data-module-id="hevy"]')?.textContent,
      ).toContain("Newest module"),
    );
    await transport.resolveSnapshot(0);

    await vi.waitFor(() =>
      expect(
        target.querySelector('[data-module-id="hevy"]')?.textContent,
      ).toContain("Newest module"),
    );
    expect(
      target.querySelector('[data-module-id="hevy"]')?.textContent,
    ).not.toContain("Older module");
    unmount(app);
  });

  it("hydrates an already persisted provider on initial load", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const app = mount(SettingsPage, {
      target,
      props: {
        transport: new PersistedProviderTransport({
          catalog,
          workspace,
          bootstrap: {
            productName: "MyFitAnalytics",
            locale: "en-US",
            activeProviders: { "body.weight": "hevy" },
            modules: [],
          },
        }),
      },
    });

    await vi.waitFor(() =>
      expect(target.textContent).toContain("Active provider"),
    );
    expect(
      target.querySelector('[data-module-id="hevy"]')?.textContent,
    ).toContain("Active provider");
    expect(
      target.querySelector('[data-module-id="mynetdiary"]')?.textContent,
    ).not.toContain("Active provider");
    unmount(app);
  });

  it("rehydrates the selected provider after a Settings remount", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const transport = new PersistedProviderTransport({ catalog, workspace });
    const app = mount(SettingsPage, { target, props: { transport } });

    await vi.waitFor(() => expect(target.textContent).toContain("Hevy"));
    target
      .querySelector<HTMLButtonElement>(
        '[data-module-id="hevy"][data-action="provider"]',
      )
      ?.click();
    await vi.waitFor(() =>
      expect(target.textContent).toContain("Active provider"),
    );
    unmount(app);

    const remounted = mount(SettingsPage, { target, props: { transport } });
    await vi.waitFor(() =>
      expect(target.textContent).toContain("Active provider"),
    );
    expect(
      target.querySelector('[data-module-id="hevy"]')?.textContent,
    ).toContain("Active provider");
    unmount(remounted);
  });

  it("renders bootstrap failures without exposing raw transport details", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const app = mount(SettingsPage, {
      target,
      props: { transport: new BootstrapErrorTransport({ catalog, workspace }) },
    });

    await vi.waitFor(() =>
      expect(target.textContent).toContain(
        "The module action could not be completed.",
      ),
    );
    expect(target.textContent).toContain("bootstrap_failed");
    expect(target.textContent).not.toContain("raw bootstrap failure detail");
    unmount(app);
  });

  it("groups bundled sources and renders available, enabled, and error states", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const app = mount(SettingsPage, {
      target,
      props: { transport: new MockTransport({ catalog, workspace }) },
    });

    await vi.waitFor(() => expect(target.textContent).toContain("Hevy"));
    expect(target.textContent).toContain("Hevy");
    expect(target.textContent).toContain("MyNetDiary");
    expect(target.textContent).toContain("Sources");
    expect(target.textContent).toContain("Dashboards");
    expect(target.textContent).toContain("Language");
    expect(target.textContent).toContain("Available");
    expect(target.textContent).toContain("Enabled");
    expect(target.textContent).toContain("Incompatible");
    expect(target.textContent).toContain("incompatible_source_api");
    expect(target.textContent).toContain("/tmp/health-workspace/inbox/hevy");
    unmount(app);
  });

  it("treats workspace and package dialog cancellation as a no-op", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const transport = new MockTransport({
      catalog,
      chooseWorkspaceResult: null,
      chooseAndInstallResult: null,
    });
    const app = mount(SettingsPage, { target, props: { transport } });

    await vi.waitFor(() =>
      expect(target.textContent).toContain("Choose Workspace"),
    );
    target
      .querySelector<HTMLButtonElement>('[data-action="choose-workspace"]')
      ?.click();
    target
      .querySelector<HTMLButtonElement>('[data-action="install-package"]')
      ?.click();
    await vi.waitFor(() =>
      expect(target.textContent).not.toContain("Unable to"),
    );
    expect(target.textContent).toContain("Choose Workspace");
    unmount(app);
  });

  it("refreshes catalog after lifecycle actions and keeps provider controls explicit", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const transport = new MockTransport({ catalog, workspace });
    const app = mount(SettingsPage, { target, props: { transport } });

    await vi.waitFor(() => expect(target.textContent).toContain("Hevy"));
    target
      .querySelector<HTMLButtonElement>(
        '[data-module-id="hevy"][data-action="disable"]',
      )
      ?.click();
    await vi.waitFor(() =>
      expect(transport.calls).toContain("setModuleEnabled:hevy:false"),
    );
    target
      .querySelector<HTMLButtonElement>(
        '[data-module-id="hevy"][data-action="provider"]',
      )
      ?.click();
    await vi.waitFor(() =>
      expect(transport.calls).toContain("selectProvider:body.weight:hevy"),
    );
    await vi.waitFor(() =>
      expect(target.textContent).toContain("Active provider"),
    );
    target
      .querySelector<HTMLButtonElement>(
        '[data-module-id="hevy"][data-action="choose-inbox"]',
      )
      ?.click();
    await vi.waitFor(() =>
      expect(transport.calls).toContain("chooseSourceInbox:hevy"),
    );
    unmount(app);
  });

  it("renders a localized lifecycle failure without exposing a path field", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const app = mount(SettingsPage, {
      target,
      props: {
        transport: new MockTransport({
          error: { code: "workspace_required", message: "raw native detail" },
        }),
      },
    });

    await vi.waitFor(() =>
      expect(target.textContent).toContain(
        "Configure a workspace before enabling this source.",
      ),
    );
    expect(target.querySelectorAll("input")).toHaveLength(0);
    expect(target.textContent).not.toContain("raw native detail");
    unmount(app);
  });

  it("requires confirmation before uninstalling a disabled module", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const transport = new MockTransport({ catalog, workspace });
    const app = mount(SettingsPage, { target, props: { transport } });

    await vi.waitFor(() => expect(target.textContent).toContain("Dashboards"));
    target
      .querySelector<HTMLButtonElement>(
        '[data-module-id="summary-dashboard"][data-action="uninstall"]',
      )
      ?.click();
    await vi.waitFor(() =>
      expect(target.textContent).toContain(
        "Are you sure you want to uninstall",
      ),
    );
    expect(transport.calls).not.toContain("uninstallModule:summary-dashboard");
    target
      .querySelector<HTMLButtonElement>('[data-action="confirm-uninstall"]')
      ?.click();
    await vi.waitFor(() =>
      expect(transport.calls).toContain("uninstallModule:summary-dashboard"),
    );
    unmount(app);
  });
});
