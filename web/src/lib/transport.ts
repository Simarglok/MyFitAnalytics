import type {
  AttemptView,
  BootstrapState,
  DashboardPageView,
  DateRangeView,
  IngestionStatus,
  ModuleCatalogEntry,
  ModuleView,
  QualityItem,
  ScanTicket,
  DataChangedEvent,
  NavigationView,
  PhaseEventInput,
  PhaseEventView,
  ProviderView,
  WorkspaceView,
} from "./types";

export interface AppTransport {
  getBootstrapState(): Promise<BootstrapState>;
  getNavigation(): Promise<NavigationView>;
  getDashboard(
    moduleId: string,
    pageId: string,
    range: DateRangeView,
  ): Promise<DashboardPageView>;
  listModules(): Promise<ModuleView[]>;
  refreshNow(): Promise<ScanTicket>;
  getIngestionStatus(): Promise<IngestionStatus>;
  listQualityItems(): Promise<QualityItem[]>;
  retryAsset(assetId: string): Promise<AttemptView>;
  listModuleCatalog?(): Promise<ModuleCatalogEntry[]>;
  getWorkspaceView?(): Promise<WorkspaceView>;
  chooseWorkspaceRoot?(): Promise<WorkspaceView | null>;
  chooseAndInstallModule?(): Promise<ModuleView | null>;
  chooseSourceInbox?(moduleId: string): Promise<WorkspaceView | null>;
  setModuleEnabled?(moduleId: string, enabled: boolean): Promise<ModuleView>;
  updateModule?(moduleId: string): Promise<ModuleView>;
  uninstallModule?(moduleId: string): Promise<void>;
  selectModuleProvider?(
    capability: string,
    moduleId: string,
  ): Promise<ProviderSelection>;
  selectProvider?(capability: string, moduleId: string): Promise<ProviderView>;
  listPhaseEvents(): Promise<PhaseEventView[]>;
  savePhaseEvent(input: PhaseEventInput): Promise<PhaseEventView>;
  deletePhaseEvent(phaseEventId: string): Promise<void>;
  subscribeDataChanged(
    listener: (event: DataChangedEvent) => void,
  ): Promise<() => void>;
}

export interface ProviderSelection {
  activeProviders: Record<string, string>;
}

export interface SerializedCommandError {
  code: string;
  message: string;
}

export class TransportError extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = "TransportError";
    this.code = code;
  }
}

export function normalizeTransportError(error: unknown): TransportError {
  if (typeof error === "object" && error !== null) {
    const candidate = error as Partial<SerializedCommandError>;
    if (
      typeof candidate.code === "string" &&
      typeof candidate.message === "string"
    ) {
      return new TransportError(candidate.code, candidate.message);
    }
  }
  if (error instanceof Error)
    return new TransportError("transport_error", error.message);
  return new TransportError(
    "transport_error",
    "The desktop transport is unavailable.",
  );
}
