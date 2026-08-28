export type AvailabilityState =
  | "missing_capability"
  | "missing_dependency"
  | "incompatible_contract"
  | "waiting_for_data"
  | "insufficient_coverage"
  | "ready"
  | "disabled_by_user";

export interface AvailabilityView {
  state: AvailabilityState;
  reasonKey: string;
  requiredCapabilities: string[];
  requiredDependencies: string[];
  action: string | null;
}

export type DashboardSummaryValue = string | number | boolean;

export interface DashboardCardPresentation {
  summaryKey: string;
  summaryValue?: DashboardSummaryValue | null;
}

export type ChartPoint = [label: string, value: number | null];

export interface ChartSeries {
  name: string;
  points: ChartPoint[];
}

export interface DashboardCard {
  key: string;
  label: string;
  value: unknown;
  presentation?: DashboardCardPresentation;
}

export interface DashboardTable {
  key: string;
  columns: string[];
  rows: unknown[][];
}

export interface DashboardStatusPanel {
  key: string;
  state: AvailabilityView | { type: AvailabilityState };
  messageKey: string;
}

export interface DashboardChart {
  key: string;
  chartType: "line" | "bar" | "scatter" | "calendar_heatmap";
  series: ChartSeries[];
}

export type DashboardBlock =
  | { type: "card"; value: DashboardCard }
  | { type: "table"; value: DashboardTable }
  | { type: "status_panel"; value: DashboardStatusPanel }
  | { type: "chart"; value: DashboardChart };

export interface DashboardDocument {
  titleKey: string;
  blocks: DashboardBlock[];
}

export interface ModuleErrorView {
  code: string;
  messageKey: string;
}

export type DashboardOutput = DashboardDocument | ModuleErrorView;

export interface CoverageView {
  expectedDays: number;
  observedDays: number;
  ratio: number;
  sufficient: boolean;
}

export interface FreshnessView {
  latestObservationDate: string | null;
  generatedAt: string;
}

export interface NavigationItemView {
  id: string;
  pageId: string;
  titleKey: string;
  moduleId: string;
  availability: AvailabilityView;
}

export interface NavigationView {
  items: NavigationItemView[];
  initialRange: DateRangeView;
}

export interface DashboardPageView {
  moduleId: string;
  pageId: string;
  titleKey: string;
  document: DashboardOutput;
  availability: AvailabilityView;
  coverage: CoverageView;
  freshness: FreshnessView;
}

export interface ProviderView {
  capability: string;
  moduleId: string;
  activeProviders: Record<string, string>;
}

export interface DateRangeView {
  start: string;
  end: string;
}

export interface PhaseEventInput {
  phaseEventId: string | null;
  eventType: string;
  startDate: string;
  endDate: string;
  description: string | null;
  excludeFromTdee: boolean;
}

export interface PhaseEventView extends Omit<PhaseEventInput, "phaseEventId"> {
  phaseEventId: string;
}

export interface ModuleView {
  id: string;
  moduleType: "source" | "dashboard" | "locale";
  version: string;
  enabled: boolean;
  localizationNamespace: string;
  displayName?: string;
  providedCapabilities?: string[];
}

export type ModuleInstallState =
  "available" | "enabled" | "disabled" | "update" | "error" | "incompatible";

export interface ModuleCatalogEntry {
  module: ModuleView;
  origin: "bundled" | "installed";
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

export type HealthState = "healthy" | "working" | "attention" | "blocked";

export interface HealthSummary {
  state: HealthState;
  workingJobs: number;
  waitingAssets: number;
  attentionItems: number;
  criticalItems: number;
  failureCodeCounts: Record<string, number>;
}

export interface IngestionStatus {
  health: HealthSummary;
  queueCapacity: number;
  recoveryMode: "normal" | "recovery" | "unconfigured";
  configured: boolean;
  pendingModuleUpdates: string[];
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
