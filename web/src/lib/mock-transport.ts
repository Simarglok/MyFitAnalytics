import { normalizeTransportError } from './transport';
import type { AppTransport } from './transport';
import type {
  AttemptView,
  BootstrapState,
  IngestionStatus,
  ModuleCatalogEntry,
  ModuleView,
  QualityItem,
  ScanTicket,
  DataChangedEvent,
  WorkspaceView,
} from './types';
import type { ProviderSelection } from './transport';

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
}

export class MockTransport implements AppTransport {
  private readonly options: MockTransportOptions;
  private catalogState: ModuleCatalogEntry[];
  readonly calls: string[] = [];

  constructor(options: MockTransportOptions = {}) {
    this.options = options;
    this.catalogState = options.catalog ?? [];
  }

  async getBootstrapState(): Promise<BootstrapState> {
    if (this.options.error !== undefined) throw normalizeTransportError(this.options.error);
    if (this.options.bootstrapPromise) return this.options.bootstrapPromise;
    if (this.options.bootstrap) return this.options.bootstrap;
    return {
      productName: 'MyFitAnalytics',
      locale: 'en-US',
      activeProviders: {},
      modules: this.options.modules ?? [],
    };
  }

  async listModules(): Promise<ModuleView[]> {
    if (this.options.error !== undefined) throw normalizeTransportError(this.options.error);
    return this.options.modules ?? this.options.bootstrap?.modules ?? [];
  }


  async listModuleCatalog(): Promise<ModuleCatalogEntry[]> {
    if (this.options.error !== undefined) throw normalizeTransportError(this.options.error);
    return this.catalogState;
  }

  async chooseWorkspaceRoot(): Promise<WorkspaceView | null> {
    if (this.options.error !== undefined) throw normalizeTransportError(this.options.error);
    this.calls.push('chooseWorkspaceRoot');
    if (Object.prototype.hasOwnProperty.call(this.options, 'chooseWorkspaceResult')) {
      return this.options.chooseWorkspaceResult ?? null;
    }
    return this.options.workspace ?? null;
  }

  async chooseAndInstallModule(): Promise<ModuleView | null> {
    if (this.options.error !== undefined) throw normalizeTransportError(this.options.error);
    this.calls.push('chooseAndInstallModule');
    if (Object.prototype.hasOwnProperty.call(this.options, 'chooseAndInstallResult')) {
      return this.options.chooseAndInstallResult ?? null;
    }
    return null;
  }

  async getWorkspaceView(): Promise<WorkspaceView> {
    if (this.options.error !== undefined) throw normalizeTransportError(this.options.error);
    return (
      this.options.workspace ?? {
        workspaceRoot: '',
        appDataRoot: '/tmp/app-data',
        databasePath: '/tmp/app-data/myfitanalytics.duckdb',
        recoveryPath: '/tmp/app-data/recovery',
        backupPath: '/tmp/app-data/recovery',
        archiveRoot: '',
        sourcePaths: [],
      }
    );
  }

  async chooseSourceInbox(moduleId: string): Promise<WorkspaceView | null> {
    if (this.options.error !== undefined) throw normalizeTransportError(this.options.error);
    this.calls.push(`chooseSourceInbox:${moduleId}`);
    return this.options.workspace ?? null;
  }

  async setModuleEnabled(moduleId: string, enabled: boolean): Promise<ModuleView> {
    if (this.options.error !== undefined) throw normalizeTransportError(this.options.error);
    this.calls.push(`setModuleEnabled:${moduleId}:${enabled}`);
    const entry = this.catalogState.find((candidate) => candidate.module.id === moduleId);
    if (!entry) throw normalizeTransportError({ code: 'module_not_found', message: 'Module not found' });
    const module = { ...entry.module, enabled };
    this.catalogState = this.catalogState.map((candidate) =>
      candidate.module.id === moduleId
        ? { ...candidate, module, installState: enabled ? 'enabled' : 'disabled' }
        : candidate,
    );
    return module;
  }

  async updateModule(moduleId: string): Promise<ModuleView> {
    if (this.options.error !== undefined) throw normalizeTransportError(this.options.error);
    this.calls.push(`updateModule:${moduleId}`);
    const entry = this.catalogState.find((candidate) => candidate.module.id === moduleId);
    if (!entry) throw normalizeTransportError({ code: 'module_not_found', message: 'Module not found' });
    return entry.module;
  }

  async uninstallModule(moduleId: string): Promise<void> {
    if (this.options.error !== undefined) throw normalizeTransportError(this.options.error);
    this.calls.push(`uninstallModule:${moduleId}`);
    this.catalogState = this.catalogState.filter((candidate) => candidate.module.id !== moduleId);
  }

  async selectModuleProvider(capability: string, moduleId: string): Promise<ProviderSelection> {
    if (this.options.error !== undefined) throw normalizeTransportError(this.options.error);
    this.calls.push(`selectProvider:${capability}:${moduleId}`);
    return { activeProviders: { [capability]: moduleId } };
  }

  async refreshNow(): Promise<ScanTicket> {
    if (this.options.error !== undefined) throw normalizeTransportError(this.options.error);
    return { scanId: 'mock-scan', coalescedRequests: 0 };
  }

  async getIngestionStatus(): Promise<IngestionStatus> {
    if (this.options.error !== undefined) throw normalizeTransportError(this.options.error);
    return (
      this.options.status ?? {
        health: {
          state: 'healthy',
          workingJobs: 0,
          waitingAssets: 0,
          attentionItems: 0,
          criticalItems: 0,
        },
        queueCapacity: 32,
        recoveryMode: 'unconfigured',
        configured: false,
      }
    );
  }

  async listQualityItems(): Promise<QualityItem[]> {
    if (this.options.error !== undefined) throw normalizeTransportError(this.options.error);
    return this.options.qualityItems ?? [];
  }

  async retryAsset(assetId: string): Promise<AttemptView> {
    if (this.options.error !== undefined) throw normalizeTransportError(this.options.error);
    return { assetId, attemptId: null, status: 'retry_queued', errorCode: null };
  }

  async subscribeDataChanged(_listener: (event: DataChangedEvent) => void): Promise<() => void> {
    return () => undefined;
  }
}
