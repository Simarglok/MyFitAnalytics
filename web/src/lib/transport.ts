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

export interface AppTransport {
  getBootstrapState(): Promise<BootstrapState>;
  listModules(): Promise<ModuleView[]>;
  setWorkspaceRoot(path: string): Promise<WorkspaceView>;
  refreshNow(): Promise<ScanTicket>;
  getIngestionStatus(): Promise<IngestionStatus>;
  listQualityItems(): Promise<QualityItem[]>;
  retryAsset(assetId: string): Promise<AttemptView>;
  subscribeDataChanged(listener: (event: DataChangedEvent) => void): Promise<() => void>;
}

export interface SerializedCommandError {
  code: string;
  message: string;
}

export class TransportError extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = 'TransportError';
    this.code = code;
  }
}

export function normalizeTransportError(error: unknown): TransportError {
  if (typeof error === 'object' && error !== null) {
    const candidate = error as Partial<SerializedCommandError>;
    if (typeof candidate.code === 'string' && typeof candidate.message === 'string') {
      return new TransportError(candidate.code, candidate.message);
    }
  }
  if (error instanceof Error) return new TransportError('transport_error', error.message);
  return new TransportError('transport_error', 'The desktop transport is unavailable.');
}
