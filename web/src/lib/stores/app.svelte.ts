import type { AppTransport } from "../transport";
import { normalizeTransportError, type TransportError } from "../transport";
import type {
  BootstrapState,
  DataChangedEvent,
  IngestionStatus,
  NavigationView,
} from "../types";

export interface AppStoreState {
  bootstrap: BootstrapState | null;
  navigation: NavigationView | null;
  ingestionStatus: IngestionStatus | null;
  loading: boolean;
  refreshing: boolean;
  error: TransportError | null;
  selectedPageId: string;
  selectedModuleId: string;
  dataChanged: DataChangedEvent | null;
}

export function createAppStore(transport: AppTransport) {
  const state = $state<AppStoreState>({
    bootstrap: null,
    navigation: null,
    ingestionStatus: null,
    loading: true,
    refreshing: false,
    error: null,
    selectedPageId: "overview",
    selectedModuleId: "base",
    dataChanged: null,
  });
  let loadGeneration = 0;

  async function load(): Promise<void> {
    const generation = ++loadGeneration;
    state.loading = state.bootstrap === null;
    state.error = null;
    try {
      const [bootstrap, navigation, ingestionStatus] = await Promise.all([
        transport.getBootstrapState(),
        transport.getNavigation(),
        transport.getIngestionStatus(),
      ]);
      if (generation !== loadGeneration) return;
      state.bootstrap = bootstrap;
      state.navigation = navigation;
      state.ingestionStatus = ingestionStatus;
      state.loading = false;
      const selected =
        navigation.items.find((item) => item.pageId === state.selectedPageId) ??
        navigation.items[0];
      if (selected) {
        state.selectedPageId = selected.pageId;
        state.selectedModuleId = selected.moduleId;
      }
    } catch (error: unknown) {
      if (generation !== loadGeneration) return;
      state.error = normalizeTransportError(error);
      state.loading = false;
    }
  }

  async function refresh(): Promise<void> {
    state.refreshing = true;
    try {
      await transport.refreshNow();
      await load();
    } finally {
      state.refreshing = false;
    }
  }

  function select(pageId: string, moduleId = "base"): void {
    state.selectedPageId = pageId;
    state.selectedModuleId = moduleId;
  }

  async function subscribe(): Promise<() => void> {
    return transport.subscribeDataChanged((event) => {
      state.dataChanged = event;
      void load();
    });
  }

  return { state, load, refresh, select, subscribe };
}
