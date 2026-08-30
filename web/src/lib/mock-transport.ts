import { normalizeTransportError } from "./transport";
import type { AppTransport } from "./transport";
import type {
  AttemptView,
  AvailabilityState,
  AvailabilityView,
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
  listPhaseEventsError?: unknown;
  savePhaseEventError?: unknown;
  deletePhaseEventError?: unknown;
  workspace?: WorkspaceView;
  status?: IngestionStatus;
  qualityItems?: QualityItem[];
  navigation?: NavigationView;
  dashboards?: Record<string, DashboardPageView>;
  phaseEvents?: PhaseEventView[];
  dashboardAvailability?: AvailabilityState;
  health?: HealthState;
}

function availabilityAction(state: AvailabilityState): string | null {
  switch (state) {
    case "ready":
      return null;
    case "disabled_by_user":
      return "dashboard.action.enable";
    case "incompatible_contract":
      return "dashboard.action.update_module";
    case "missing_capability":
    case "missing_dependency":
      return "dashboard.action.configure_source";
    case "waiting_for_data":
    case "insufficient_coverage":
      return "dashboard.action.import_data";
  }
}

function availabilityView(state: AvailabilityState): AvailabilityView {
  return {
    state,
    reasonKey: `dashboard.${state}`,
    requiredCapabilities: [],
    requiredDependencies: [],
    action: availabilityAction(state),
  };
}

export class MockTransport implements AppTransport {
  private readonly options: MockTransportOptions;
  private catalogState: ModuleCatalogEntry[];
  private readonly phaseEvents: PhaseEventView[];
  private activeProvidersState: Record<string, string>;
  private bootstrapLoaded = false;
  readonly calls: string[] = [];

  constructor(options: MockTransportOptions = {}) {
    this.options = options;
    this.catalogState = options.catalog ?? [];
    this.phaseEvents = options.phaseEvents ? [...options.phaseEvents] : [];
    this.activeProvidersState = {
      ...(options.bootstrap?.activeProviders ?? {}),
    };
  }

  async getBootstrapState(): Promise<BootstrapState> {
    if (this.options.error !== undefined)
      throw normalizeTransportError(this.options.error);
    const bootstrap = this.options.bootstrapPromise
      ? await this.options.bootstrapPromise
      : (this.options.bootstrap ?? {
          productName: "MyFitAnalytics",
          locale: "en-US",
          activeProviders: {},
          modules: this.options.modules ?? [],
        });
    if (!this.bootstrapLoaded) {
      this.activeProvidersState = { ...bootstrap.activeProviders };
      this.bootstrapLoaded = true;
    }
    return {
      ...bootstrap,
      activeProviders: { ...this.activeProvidersState },
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
    const state = this.options.dashboardAvailability ?? "ready";
    return {
      items: pages.map(([pageId]) => ({
        id: `base:${pageId}`,
        pageId,
        titleKey: `base.${pageId}.title`,
        moduleId: "base",
        availability: availabilityView(state),
      })),
      initialRange: { start: "2026-01-01", end: "2026-01-31" },
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
    const state = this.options.dashboardAvailability ?? "ready";
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
      availability: availabilityView(state),
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
    this.activeProvidersState = {
      ...this.activeProvidersState,
      [capability]: moduleId,
    };
    return { activeProviders: { ...this.activeProvidersState } };
  }

  async selectProvider(
    capability: string,
    moduleId: string,
  ): Promise<ProviderView> {
    if (this.options.error !== undefined)
      throw normalizeTransportError(this.options.error);
    this.calls.push(`selectProvider:${capability}:${moduleId}`);
    this.activeProvidersState = {
      ...this.activeProvidersState,
      [capability]: moduleId,
    };
    return {
      capability,
      moduleId,
      activeProviders: { ...this.activeProvidersState },
    };
  }

  async savePhaseEvent(input: PhaseEventInput): Promise<PhaseEventView> {
    if (this.options.error !== undefined)
      throw normalizeTransportError(this.options.error);
    if (this.options.savePhaseEventError !== undefined)
      throw normalizeTransportError(this.options.savePhaseEventError);
    const event: PhaseEventView = {
      ...input,
      phaseEventId:
        input.phaseEventId ?? `mock-phase-${this.phaseEvents.length + 1}`,
    };
    const existing = this.phaseEvents.findIndex(
      (candidate) => candidate.phaseEventId === event.phaseEventId,
    );
    if (existing === -1) this.phaseEvents.push(event);
    else this.phaseEvents[existing] = event;
    this.calls.push(`savePhaseEvent:${event.phaseEventId}`);
    return event;
  }

  async listPhaseEvents(): Promise<PhaseEventView[]> {
    if (this.options.error !== undefined)
      throw normalizeTransportError(this.options.error);
    if (this.options.listPhaseEventsError !== undefined)
      throw normalizeTransportError(this.options.listPhaseEventsError);
    this.calls.push("listPhaseEvents");
    return this.phaseEvents.map((event) => ({ ...event }));
  }

  async deletePhaseEvent(phaseEventId: string): Promise<void> {
    if (this.options.error !== undefined)
      throw normalizeTransportError(this.options.error);
    if (this.options.deletePhaseEventError !== undefined)
      throw normalizeTransportError(this.options.deletePhaseEventError);
    const existing = this.phaseEvents.findIndex(
      (event) => event.phaseEventId === phaseEventId,
    );
    if (existing === -1) {
      throw {
        code: "phase_event_not_found",
        message: "the phase event no longer exists",
      };
    }
    this.phaseEvents.splice(existing, 1);
    this.calls.push(`deletePhaseEvent:${phaseEventId}`);
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
          failureCodeCounts: {},
        },
        queueCapacity: 32,
        recoveryMode: "unconfigured",
        configured: false,
        pendingModuleUpdates: [],
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
