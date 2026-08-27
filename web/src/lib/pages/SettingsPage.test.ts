import { mount, unmount } from "svelte";
import { describe, expect, it, vi } from "vitest";
import SettingsPage from "./SettingsPage.svelte";
import { MockTransport } from "../mock-transport";
import type { ModuleCatalogEntry, WorkspaceView } from "../types";

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

describe("SettingsPage", () => {
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
