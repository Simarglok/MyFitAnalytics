import type { AppTransport } from "../transport";
import { normalizeTransportError, type TransportError } from "../transport";
import type { DashboardPageView, DateRangeView } from "../types";

export interface DashboardStoreState {
  page: DashboardPageView | null;
  range: DateRangeView;
  loading: boolean;
  error: TransportError | null;
  stale: boolean;
}

export interface DashboardStore {
  state: DashboardStoreState;
  load(moduleId: string, pageId: string, range?: DateRangeView): Promise<void>;
  markStale(): void;
}

export function createDashboardStore(transport: AppTransport): DashboardStore {
  const state = $state<DashboardStoreState>({
    page: null,
    range: { start: "2026-01-01", end: "2026-01-31" },
    loading: false,
    error: null,
    stale: false,
  });
  let requestGeneration = 0;

  async function load(
    moduleId: string,
    pageId: string,
    range = state.range,
  ): Promise<void> {
    const generation = ++requestGeneration;
    state.range = { ...range };
    state.loading = true;
    state.error = null;
    try {
      const page = await transport.getDashboard(moduleId, pageId, range);
      if (generation !== requestGeneration) return;
      state.page = page;
      state.stale = false;
      state.loading = false;
    } catch (error: unknown) {
      if (generation !== requestGeneration) return;
      state.error = normalizeTransportError(error);
      state.loading = false;
    }
  }

  function markStale(): void {
    state.stale = true;
  }

  return { state, load, markStale };
}
