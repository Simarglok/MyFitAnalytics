import { describe, expect, it } from 'vitest';
import type { AppTransport } from './lib/transport';
import type { DataChangedEvent, IngestionStatus, QualityItem, WorkspaceView } from './lib/types';

const workspace: WorkspaceView = {
  workspaceRoot: '/tmp/workspace',
  appDataRoot: '/tmp/app-data',
  databasePath: '/tmp/app-data/myfitanalytics.duckdb',
  recoveryPath: '/tmp/app-data/recovery',
  backupPath: '/tmp/app-data/recovery',
  archiveRoot: '/tmp/workspace/archive',
  sourcePaths: [],
};

const status: IngestionStatus = {
  health: {
    state: 'healthy',
    workingJobs: 0,
    waitingAssets: 0,
    attentionItems: 0,
    criticalItems: 0,
  },
  queueCapacity: 32,
  recoveryMode: 'normal',
  configured: true,
};

describe('storage transport boundary', () => {
  it('keeps storage commands typed and row payloads out of status events', async () => {
    const quality: QualityItem[] = [];
    const transport: AppTransport = {
      getBootstrapState: async () => ({
        productName: 'MyFitAnalytics',
        locale: 'en-US',
        activeProviders: {},
        modules: [],
      }),
      listModules: async () => [],
      chooseWorkspaceRoot: async () => workspace,
      refreshNow: async () => ({ scanId: 'scan-1', coalescedRequests: 0 }),
      getIngestionStatus: async () => status,
      listQualityItems: async () => quality,
      retryAsset: async (assetId) => ({
        assetId,
        attemptId: null,
        status: 'retry_queued',
        errorCode: null,
      }),
      subscribeDataChanged: async (listener) => {
        listener({ capabilities: ['body.weight'], dashboards: [] });
        return () => undefined;
      },
    };

    expect((await transport.chooseWorkspaceRoot?.())?.archiveRoot).toContain('archive');
    expect((await transport.refreshNow()).scanId).toBe('scan-1');
    expect((await transport.getIngestionStatus()).health.state).toBe('healthy');
    expect(await transport.listQualityItems()).toEqual([]);
    expect((await transport.retryAsset('asset-1')).attemptId).toBeNull();
    let event: DataChangedEvent | undefined;
    await transport.subscribeDataChanged((payload) => {
      event = payload;
    });
    expect(event?.capabilities).toEqual(['body.weight']);
  });
});
