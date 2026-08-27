import { normalizeTransportError } from "./transport";
import type { AppTransport } from "./transport";
import type {
  AttemptView,
  AvailabilityState,
  BootstrapState,
  DashboardPageView,
  DateRangeView,
  IngestionStatus,
  ModuleCatalogEntry,
  ModuleView,
  QualityItem,
  ScanTicket,
  DataChangedEvent,
  HealthState,
  NavigationView,
  PhaseEventInput,
  PhaseEventView,
  ProviderView,
  WorkspaceView,
} from "./types";
import type { ProviderSelection } from "./transport";

export interface MockTransportOptions {
  bootstrap?: BootstrapState;
  bootstrapPromise?: Promise<BootstrapState>;
  modules?: ModuleView[];
  catalog?: ModuleCatalogEntry[];
  chooseWorkspaceResult?: WorkspaceView | null;
  chooseAndInstallResult?: ModuleView | null;
  error?: unknown;
  workspace?: WorkspaceView;
  status?: IngestionStatus;
  qualityItems?: QualityItem[];
  navigation?: NavigationView;
  dashboards?: Record<string, DashboardPageView>;
  phaseEvents?: PhaseEventView[];
  dashboardAvailability?: AvailabilityState;
  health?: HealthState;
}

export class MockTransport implements AppTransport {
  private readonly options: MockTransportOptions;
  private catalogState: ModuleCatalogEntry[];
  private readonly phaseEvents: PhaseEventView[];
  readonly calls: string[] = [];

  constructor(options: MockTransportOptions = {}) {
    this.options = options;
    this.catalogState = options.catalog ?? [];
    this.phaseEvents = options.phaseEvents ? [...options.phaseEvents] : [];
  }

  async getBootstrapState(): Promise<BootstrapState> {
    if (this.options.error !== undefined)
      throw normalizeTransportError(this.options.error);
    if (this.options.bootstrapPromise) return this.options.bootstrapPromise;
    if (this.options.bootstrap) return this.options.bootstrap;
    return {
      productName: "MyFitAnalytics",
      locale: "en-US",
      activeProviders: {},
      modules: this.options.modules ?? [],
    };
  }

  async getNavigation(): Promise<NavigationView> {
    if (this.options.error !== undefined)
      throw normalizeTransportError(this.options.error);
    if (this.options.navigation) return this.options.navigation;
    const pages: Array<[string, string]> = [
      ["overview", "Overview"],
      ["body", "Body"],
      ["nutrition", "Nutrition"],
      ["activity", "Activity"],
      ["strength", "Strength"],
      ["sources", "Sources & quality"],
    ];
    return {
      items: pages.map(([pageId]) => ({
        id: `base:${pageId}`,
        pageId,
        titleKey: `base.${pageId}.title`,
        moduleId: "base",
        availability: {
          state: this.options.dashboardAvailability ?? "ready",
          reasonKey:
            this.options.dashboardAvailability === "disabled_by_user"
              ? "dashboard.disabled_by_user"
              : "dashboard.ready",
          requiredCapabilities: [],
          requiredDependencies: [],
        },
      })),
    };
  }

  async getDashboard(
    moduleId: string,
    pageId: string,
    range: DateRangeView,
  ): Promise<DashboardPageView> {
    if (this.options.error !== undefined)
      throw normalizeTransportError(this.options.error);
    const supplied = this.options.dashboards?.[pageId];
    if (supplied) return supplied;
    return {
      moduleId,
      pageId,
      titleKey: `base.${pageId}.title`,
      document: {
        titleKey: `base.${pageId}.title`,
        blocks: [
          {
            type: "chart",
            value: {
              key: `${pageId}-trend`,
              chartType: "line",
              series: [
                {
                  name: "Weight",
                  points: [
                    [range.start, 72.4],
                    [range.end, null],
                  ],
                },
              ],
            },
          },
        ],
      },
      availability: {
        state: this.options.dashboardAvailability ?? "ready",
        reasonKey:
          this.options.dashboardAvailability === "disabled_by_user"
            ? "dashboard.disabled_by_user"
            : "dashboard.ready",
        requiredCapabilities: [],
        requiredDependencies: [],
      },
      coverage: {
        expectedDays: 30,
        observedDays: 30,
        ratio: 1,
        sufficient: true,
      },
      freshness: {
        latestObservationDate: range.end,
        generatedAt: "2026-01-31T00:00:00Z",
      },
    };
  }

  async listModules(): Promise<ModuleView[]> {
    if (this.options.error !== undefined)
      throw normalizeTransportError(this.options.error);
    return this.options.modules ?? this.options.bootstrap?.modules ?? [];
  }

  async listModuleCatalog(): Promise<ModuleCatalogEntry[]> {
    if (this.options.error !== undefined)
      throw normalizeTransportError(this.options.error);
    return this.catalogState;
  }

  async chooseWorkspaceRoot(): Promise<WorkspaceView | null> {
    if (this.options.error !== undefined)
      throw normalizeTransportError(this.options.error);
    this.calls.push("chooseWorkspaceRoot");
    if (
      Object.prototype.hasOwnProperty.call(
        this.options,
        "chooseWorkspaceResult",
      )
    ) {
      return this.options.chooseWorkspaceResult ?? null;
    }
    return this.options.workspace ?? null;
  }

  async chooseAndInstallModule(): Promise<ModuleView | null> {
    if (this.options.error !== undefined)
      throw normalizeTransportError(this.options.error);
    this.calls.push("chooseAndInstallModule");
    if (
      Object.prototype.hasOwnProperty.call(
        this.options,
        "chooseAndInstallResult",
      )
    ) {
      return this.options.chooseAndInstallResult ?? null;
    }
    return null;
  }

  async getWorkspaceView(): Promise<WorkspaceView> {
    if (this.options.error !== undefined)
      throw normalizeTransportError(this.options.error);
    return (
      this.options.workspace ?? {
        workspaceRoot: "",
        appDataRoot: "/tmp/app-data",
        databasePath: "/tmp/app-data/myfitanalytics.duckdb",
        recoveryPath: "/tmp/app-data/recovery",
        backupPath: "/tmp/app-data/recovery",
        archiveRoot: "",
        sourcePaths: [],
      }
    );
  }

  async chooseSourceInbox(moduleId: string): Promise<WorkspaceView | null> {
    if (this.options.error !== undefined)
      throw normalizeTransportError(this.options.error);
    this.calls.push(`chooseSourceInbox:${moduleId}`);
    return this.options.workspace ?? null;
  }

  async setModuleEnabled(
    moduleId: string,
    enabled: boolean,
  ): Promise<ModuleView> {
    if (this.options.error !== undefined)
      throw normalizeTransportError(this.options.error);
    this.calls.push(`setModuleEnabled:${moduleId}:${enabled}`);
    const entry = this.catalogState.find(
      (candidate) => candidate.module.id === moduleId,
    );
    if (!entry)
      throw normalizeTransportError({
        code: "module_not_found",
        message: "Module not found",
      });
    const module = { ...entry.module, enabled };
    this.catalogState = this.catalogState.map((candidate) =>
      candidate.module.id === moduleId
        ? {
            ...candidate,
            module,
            installState: enabled ? "enabled" : "disabled",
          }
        : candidate,
    );
    return module;
  }

  async updateModule(moduleId: string): Promise<ModuleView> {
    if (this.options.error !== undefined)
      throw normalizeTransportError(this.options.error);
    this.calls.push(`updateModule:${moduleId}`);
    const entry = this.catalogState.find(
      (candidate) => candidate.module.id === moduleId,
    );
    if (!entry)
      throw normalizeTransportError({
        code: "module_not_found",
        message: "Module not found",
      });
    return entry.module;
  }

  async uninstallModule(moduleId: string): Promise<void> {
    if (this.options.error !== undefined)
      throw normalizeTransportError(this.options.error);
    this.calls.push(`uninstallModule:${moduleId}`);
    this.catalogState = this.catalogState.filter(
      (candidate) => candidate.module.id !== moduleId,
    );
  }

  async selectModuleProvider(
    capability: string,
    moduleId: string,
  ): Promise<ProviderSelection> {
    if (this.options.error !== undefined)
      throw normalizeTransportError(this.options.error);
    this.calls.push(`selectProvider:${capability}:${moduleId}`);
    return { activeProviders: { [capability]: moduleId } };
  }

  async selectProvider(
    capability: string,
    moduleId: string,
  ): Promise<ProviderView> {
    if (this.options.error !== undefined)
      throw normalizeTransportError(this.options.error);
    this.calls.push(`selectProvider:${capability}:${moduleId}`);
    return {
      capability,
      moduleId,
      activeProviders: { [capability]: moduleId },
    };
  }

  async savePhaseEvent(input: PhaseEventInput): Promise<PhaseEventView> {
    if (this.options.error !== undefined)
      throw normalizeTransportError(this.options.error);
    const event: PhaseEventView = {
      ...input,
      phaseEventId:
        input.phaseEventId ?? `mock-phase-${this.phaseEvents.length + 1}`,
    };
    this.phaseEvents.push(event);
    this.calls.push(`savePhaseEvent:${event.phaseEventId}`);
    return event;
  }

  async refreshNow(): Promise<ScanTicket> {
    if (this.options.error !== undefined)
      throw normalizeTransportError(this.options.error);
    return { scanId: "mock-scan", coalescedRequests: 0 };
  }

  async getIngestionStatus(): Promise<IngestionStatus> {
    if (this.options.error !== undefined)
      throw normalizeTransportError(this.options.error);
    return (
      this.options.status ?? {
        health: {
          state: this.options.health ?? "healthy",
          workingJobs: 0,
          waitingAssets: 0,
          attentionItems: 0,
          criticalItems: 0,
        },
        queueCapacity: 32,
        recoveryMode: "unconfigured",
        configured: false,
      }
    );
  }

  async listQualityItems(): Promise<QualityItem[]> {
    if (this.options.error !== undefined)
      throw normalizeTransportError(this.options.error);
    return this.options.qualityItems ?? [];
  }

  async retryAsset(assetId: string): Promise<AttemptView> {
    if (this.options.error !== undefined)
      throw normalizeTransportError(this.options.error);
    this.calls.push(`retryAsset:${assetId}`);
    return {
      assetId,
      attemptId: null,
      status: "retry_queued",
      errorCode: null,
    };
  }

  async subscribeDataChanged(
    _listener: (event: DataChangedEvent) => void,
  ): Promise<() => void> {
    return () => undefined;
  }
}
