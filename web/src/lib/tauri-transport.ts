import { invoke } from '@tauri-apps/api/core';
import { normalizeTransportError } from './transport';
import type { AppTransport } from './transport';
import type { BootstrapState, ModuleView } from './types';

export class TauriTransport implements AppTransport {
  async getBootstrapState(): Promise<BootstrapState> {
    try {
      return await invoke<BootstrapState>('get_bootstrap_state');
    } catch (error) {
      throw normalizeTransportError(error);
    }
  }

  async listModules(): Promise<ModuleView[]> {
    try {
      return await invoke<ModuleView[]>('list_modules');
    } catch (error) {
      throw normalizeTransportError(error);
    }
  }
}
