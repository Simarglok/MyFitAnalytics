import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
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

export class TauriTransport implements AppTransport {
  async getBootstrapState(): Promise<BootstrapState> {
    return this.invoke('get_bootstrap_state');
  }

  async listModules(): Promise<ModuleView[]> {
    return this.invoke('list_modules');
  }

  async setWorkspaceRoot(path: string): Promise<WorkspaceView> {
    return this.invoke('set_workspace_root', { path });
  }

  async refreshNow(): Promise<ScanTicket> {
    return this.invoke('refresh_now');
  }

  async getIngestionStatus(): Promise<IngestionStatus> {
    return this.invoke('get_ingestion_status');
  }

  async listQualityItems(): Promise<QualityItem[]> {
    return this.invoke('list_quality_items');
  }

  async retryAsset(assetId: string): Promise<AttemptView> {
    return this.invoke('retry_asset', { assetId });
  }

  async subscribeDataChanged(
    listener: (event: DataChangedEvent) => void,
  ): Promise<() => void> {
    try {
      return await listen<DataChangedEvent>('data-changed', (event) => listener(event.payload));
    } catch (error) {
      throw normalizeTransportError(error);
    }
  }

  private async invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
    try {
      return await invoke<T>(command, args);
    } catch (error) {
      throw normalizeTransportError(error);
    }
  }
}
