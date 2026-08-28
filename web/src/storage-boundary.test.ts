import { describe, expect, it } from "vitest";
import type { AppTransport } from "./lib/transport";
import type {
  DashboardPageView,
  DataChangedEvent,
  DateRangeView,
  IngestionStatus,
  NavigationView,
  PhaseEventInput,
  PhaseEventView,
  QualityItem,
  WorkspaceView,
} from "./lib/types";

const workspace: WorkspaceView = {
  workspaceRoot: "/tmp/workspace",
  appDataRoot: "/tmp/app-data",
  databasePath: "/tmp/app-data/myfitanalytics.duckdb",
  recoveryPath: "/tmp/app-data/recovery",
  backupPath: "/tmp/app-data/recovery",
  archiveRoot: "/tmp/workspace/archive",
  sourcePaths: [],
};

const status: IngestionStatus = {
  health: {
    state: "healthy",
    workingJobs: 0,
    waitingAssets: 0,
    attentionItems: 0,
    criticalItems: 0,
  },
  queueCapacity: 32,
  recoveryMode: "normal",
  configured: true,
};

describe("storage transport boundary", () => {
  it("keeps storage commands typed and row payloads out of status events", async () => {
    const quality: QualityItem[] = [];
    const transport: AppTransport = {
      getBootstrapState: async () => ({
        productName: "MyFitAnalytics",
        locale: "en-US",
        activeProviders: {},
        modules: [],
      }),
      getNavigation: async (): Promise<NavigationView> => ({
        items: [],
        initialRange: { start: "2026-01-01", end: "2026-01-31" },
      }),
      getDashboard: async (
        _moduleId: string,
        _pageId: string,
        _range: DateRangeView,
      ): Promise<DashboardPageView> => {
        throw new Error("not used by this boundary test");
      },
      listModules: async () => [],
      chooseWorkspaceRoot: async () => workspace,
      refreshNow: async () => ({ scanId: "scan-1", coalescedRequests: 0 }),
      getIngestionStatus: async () => status,
      listQualityItems: async () => quality,
      listPhaseEvents: async (): Promise<PhaseEventView[]> => [],
      savePhaseEvent: async (
        input: PhaseEventInput,
      ): Promise<PhaseEventView> => ({
        ...input,
        phaseEventId: input.phaseEventId ?? "phase-1",
      }),
      deletePhaseEvent: async (_phaseEventId: string): Promise<void> =>
        undefined,
      retryAsset: async (assetId) => ({
        assetId,
        attemptId: null,
        status: "retry_queued",
        errorCode: null,
      }),
      subscribeDataChanged: async (listener) => {
        listener({ capabilities: ["body.weight"], dashboards: [] });
        return () => undefined;
      },
    };

    expect((await transport.chooseWorkspaceRoot?.())?.archiveRoot).toContain(
      "archive",
    );
    expect((await transport.refreshNow()).scanId).toBe("scan-1");
    expect((await transport.getIngestionStatus()).health.state).toBe("healthy");
    expect(await transport.listQualityItems()).toEqual([]);
    expect((await transport.retryAsset("asset-1")).attemptId).toBeNull();
    let event: DataChangedEvent | undefined;
    await transport.subscribeDataChanged((payload) => {
      event = payload;
    });
    expect(event?.capabilities).toEqual(["body.weight"]);
  });
});
