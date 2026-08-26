import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { normalizeTransportError } from './transport';
import type { AppTransport, ProviderSelection } from './transport';
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

export class TauriTransport implements AppTransport {
  async getBootstrapState(): Promise<BootstrapState> {
    return this.invoke('get_bootstrap_state');
  }

  async listModules(): Promise<ModuleView[]> {
    return this.invoke('list_modules');
  }


  async listModuleCatalog(): Promise<ModuleCatalogEntry[]> {
    return this.invoke('list_module_catalog');
  }

  async chooseWorkspaceRoot(): Promise<WorkspaceView | null> {
    return this.invoke('choose_workspace_root');
  }

  async getWorkspaceView(): Promise<WorkspaceView> {
    return this.invoke('get_workspace_view');
  }

  async chooseAndInstallModule(): Promise<ModuleView | null> {
    return this.invoke('choose_and_install_module');
  }

  async chooseSourceInbox(moduleId: string): Promise<WorkspaceView | null> {
    return this.invoke('choose_source_inbox', { moduleId });
  }

  async setModuleEnabled(moduleId: string, enabled: boolean): Promise<ModuleView> {
    return this.invoke('set_module_enabled', { moduleId, enabled });
  }

  async updateModule(moduleId: string): Promise<ModuleView> {
    return this.invoke('update_module', { moduleId });
  }

  async uninstallModule(moduleId: string): Promise<void> {
    return this.invoke('uninstall_module', { moduleId });
  }

  async selectModuleProvider(
    capability: string,
    moduleId: string,
  ): Promise<ProviderSelection> {
    return this.invoke('select_module_provider', { capability, moduleId });
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
