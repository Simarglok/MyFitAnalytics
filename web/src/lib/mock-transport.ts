import { normalizeTransportError } from './transport';
import type { AppTransport } from './transport';
import type {
  AttemptView,
  BootstrapState,
  IngestionStatus,
  ModuleView,
  QualityItem,
  ScanTicket,
  DataChangedEvent,
  WorkspaceView,
} from './types';

export interface MockTransportOptions {
  bootstrap?: BootstrapState;
  bootstrapPromise?: Promise<BootstrapState>;
  modules?: ModuleView[];
  error?: unknown;
  workspace?: WorkspaceView;
  status?: IngestionStatus;
  qualityItems?: QualityItem[];
}

export class MockTransport implements AppTransport {
  private readonly options: MockTransportOptions;

  constructor(options: MockTransportOptions = {}) {
    this.options = options;
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

  async setWorkspaceRoot(path: string): Promise<WorkspaceView> {
    if (this.options.error !== undefined) throw normalizeTransportError(this.options.error);
    return (
      this.options.workspace ?? {
        workspaceRoot: path,
        appDataRoot: '/tmp/app-data',
        databasePath: '/tmp/app-data/myfitanalytics.duckdb',
        recoveryPath: '/tmp/app-data/recovery',
        backupPath: '/tmp/app-data/recovery',
        archiveRoot: `${path}/archive`,
        sourcePaths: [],
      }
    );
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
