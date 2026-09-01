import type { AppTransport } from "../transport";
import { normalizeTransportError, type TransportError } from "../transport";
import type { DashboardPageView, DateRangeView } from "../types";

export interface DashboardStoreState {
  page: DashboardPageView | null;
  range: DateRangeView | null;
  loading: boolean;
  error: TransportError | null;
  stale: boolean;
}

export interface DashboardStore {
  state: DashboardStoreState;
  load(moduleId: string, pageId: string, range?: DateRangeView): Promise<void>;
  syncInitialRange(range: DateRangeView): DateRangeView;
  markStale(): void;
}

function sameRange(left: DateRangeView, right: DateRangeView): boolean {
  return left.start === right.start && left.end === right.end;
}

export function createDashboardStore(
  transport: AppTransport,
  initialRange?: DateRangeView,
): DashboardStore {
  const state = $state<DashboardStoreState>({
    page: null,
    range: initialRange ? { ...initialRange } : null,
    loading: false,
    error: null,
    stale: false,
  });
  let requestGeneration = 0;
  let backendInitialRange = initialRange ? { ...initialRange } : null;

  async function load(
    moduleId: string,
    pageId: string,
    range?: DateRangeView,
  ): Promise<void> {
    const requestedRange = range ?? state.range;
    if (!requestedRange) {
      state.error = normalizeTransportError({
        code: "missing_initial_date_range",
        message: "navigation did not provide an initial dashboard date range",
      });
      state.loading = false;
      return;
    }
    const generation = ++requestGeneration;
    state.range = { ...requestedRange };
    state.loading = true;
    state.error = null;
    try {
      const page = await transport.getDashboard(
        moduleId,
        pageId,
        requestedRange,
      );
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

  function syncInitialRange(nextRange: DateRangeView): DateRangeView {
    const shouldAdopt =
      state.range === null ||
      (backendInitialRange !== null &&
        sameRange(state.range, backendInitialRange));
    backendInitialRange = { ...nextRange };
    if (shouldAdopt) state.range = { ...nextRange };
    if (state.range === null) return { ...nextRange };
    return { ...state.range };
  }

  function markStale(): void {
    state.stale = true;
  }

  return { state, load, syncInitialRange, markStale };
}
