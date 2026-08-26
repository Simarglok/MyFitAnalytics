export interface ModuleView {
  id: string;
  moduleType: 'source' | 'dashboard' | 'locale';
  version: string;
  enabled: boolean;
  localizationNamespace: string;
  displayName?: string;
  providedCapabilities?: string[];
}

export type ModuleInstallState =
  | 'available'
  | 'enabled'
  | 'disabled'
  | 'update'
  | 'error'
  | 'incompatible';

export interface ModuleCatalogEntry {
  module: ModuleView;
  origin: 'bundled' | 'installed';
  installState: ModuleInstallState;
  availableVersion: string | null;
  errorCode: string | null;
}

export interface BootstrapState {
  productName: string;
  locale: string;
  activeProviders: Record<string, string>;
  modules: ModuleView[];
}

export interface SourcePathView {
  moduleId: string;
  inboxPath: string;
  archivePath: string;
}

export interface WorkspaceView {
  workspaceRoot: string;
  appDataRoot: string;
  databasePath: string;
  recoveryPath: string;
  backupPath: string;
  archiveRoot: string;
  sourcePaths: SourcePathView[];
}

export interface ScanTicket {
  scanId: string;
  coalescedRequests: number;
}

export type HealthState = 'healthy' | 'working' | 'attention' | 'blocked';

export interface HealthSummary {
  state: HealthState;
  workingJobs: number;
  waitingAssets: number;
  attentionItems: number;
  criticalItems: number;
}

export interface IngestionStatus {
  health: HealthSummary;
  queueCapacity: number;
  recoveryMode: 'normal' | 'recovery' | 'unconfigured';
  configured: boolean;
}

export interface QualityItem {
  id: string;
  itemType: string;
  severity: string;
  message: string;
  status: string;
  assetId: string | null;
}

export interface AttemptView {
  assetId: string;
  attemptId: string | null;
  status: string;
  errorCode: string | null;
}

export interface DataChangedEvent {
  capabilities: string[];
  dashboards: string[];
}
