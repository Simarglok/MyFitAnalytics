<script lang="ts">
  import DashboardRenderer from '../components/DashboardRenderer.svelte';
  import type { NavigationItemView } from '../types';
  import type { DashboardStore } from '../stores/dashboard.svelte';
  import { message } from '../i18n';
  import { formatDate, formatNumber } from '../i18n/format';

  export let item: NavigationItemView;
  export let dashboardStore: DashboardStore;
  export let locale = 'en-US';
  export let onAvailabilityAction: ((action: string) => void) | undefined = undefined;

  let start = dashboardStore.state.range?.start ?? '';
  let end = dashboardStore.state.range?.end ?? '';
  let rangeError = '';

  async function applyRange(): Promise<void> {
    if (!start || !end || start > end) {
      rangeError = message('phases.invalid_range');
      return;
    }
    rangeError = '';
    await dashboardStore.load(item.moduleId, item.pageId, { start, end });
  }


  $: page = dashboardStore.state.page;
  $: dashboardError = dashboardStore.state.error;
  $: if (dashboardStore.state.range && !start && !end) {
    start = dashboardStore.state.range.start;
    end = dashboardStore.state.range.end;
  }
</script>

<section class="page panel" aria-labelledby="dashboard-title">
  <div class="page-heading">
    <div>
      <p class="eyebrow">{message('navigation.title')}</p>
      <h2 id="dashboard-title">{message(item.titleKey, item.pageId)}</h2>
    </div>
    <span class="freshness" class:stale={dashboardStore.state.stale}>
      {#if page}{message('dashboard.freshness')}: {formatDate(page.freshness.latestObservationDate, locale)}{/if}
    </span>
  </div>

  <form class="range-controls" aria-label="Dashboard date range" on:submit|preventDefault={() => void applyRange()}>
    <label>From <input aria-label="Range start" type="date" bind:value={start} /></label>
    <label>To <input aria-label="Range end" type="date" bind:value={end} /></label>
    <button type="submit">Apply range</button>
  </form>
  {#if rangeError}<p class="error-detail" role="alert">{rangeError}</p>{/if}
  {#if dashboardStore.state.stale}
    <p class="stale-notice" data-stale="true" role="status">{message("dashboard.stale")}</p>
  {/if}

  {#if dashboardStore.state.loading}
    <p aria-live="polite">{message('dashboard.loading')}</p>
  {:else if dashboardError}
    <section class="error" role="alert">
      <h3>{message('dashboard.error')}</h3>
      <code>{dashboardError.code}</code>
      <p>{dashboardError.message}</p>
      <button type="button" on:click={() => void applyRange()}>{message('app.retry')}</button>
    </section>
  {:else if page}
    <div class="dashboard-meta" aria-label="Dashboard coverage">
      <span>{message('dashboard.coverage')}: {formatNumber(page.coverage.observedDays, locale)} / {formatNumber(page.coverage.expectedDays, locale)}</span>
      <span>{page.coverage.sufficient ? 'Sufficient' : 'Limited'}</span>
    </div>
    <DashboardRenderer
      document={page.document}
      availability={page.availability}
      onAvailabilityAction={onAvailabilityAction}
      stale={dashboardStore.state.stale}
      {locale}
    />
  {/if}
</section>
