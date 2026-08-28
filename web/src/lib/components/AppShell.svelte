<script lang="ts">
  import { onMount } from 'svelte';
  import Navigation from './Navigation.svelte';
  import StatusBanner from './StatusBanner.svelte';
  import DashboardPage from '../pages/DashboardPage.svelte';
  import PhaseEventsPage from '../pages/PhaseEventsPage.svelte';
  import SettingsPage from '../pages/SettingsPage.svelte';
  import SourcesQualityPage from '../pages/SourcesQualityPage.svelte';
  import { message } from '../i18n';
  import type { AppTransport } from '../transport';
  import type { NavigationItemView } from '../types';
  import type { DataChangedEvent } from '../types';
  import { createAppStore } from '../stores/app.svelte';
  import { createDashboardStore } from '../stores/dashboard.svelte';

  const props = $props<{ transport: AppTransport }>();

  const appStore = createAppStore(props.transport);
  const dashboardStore = createDashboardStore(props.transport);
  let cleanup: (() => void) | undefined;

  const appState = appStore.state;
  const selectedItem = $derived(
    appState.navigation?.items.find((item) => item.pageId === appState.selectedPageId) ?? null,
  );
  const locale = $derived(appState.bootstrap?.locale ?? 'en-US');

  onMount(() => {
    let active = true;
    async function start(): Promise<void> {
      await appStore.load();
      if (!active) return;
      const item = appStore.state.navigation?.items[0];
      const initialRange = appStore.state.navigation?.initialRange;
      if (item && initialRange) await dashboardStore.load(item.moduleId, item.pageId, initialRange);
      if (!active) return;
      cleanup = await props.transport.subscribeDataChanged((event: DataChangedEvent) => {
        appStore.state.dataChanged = event;
        dashboardStore.markStale();
        void appStore.load();
        const current = appStore.state.navigation?.items.find(
          (candidate) => candidate.pageId === appStore.state.selectedPageId,
        );
        if (current && dashboardStore.state.range) {
          void dashboardStore.load(current.moduleId, current.pageId, dashboardStore.state.range);
        }
      });
    }
    void start();
    return () => {
      active = false;
      cleanup?.();
    };
  });

  function openPage(item: NavigationItemView): void {
    appStore.select(item.pageId, item.moduleId);
    if (
      item.pageId !== 'sources' &&
      item.pageId !== 'phases' &&
      item.pageId !== 'settings' &&
      dashboardStore.state.range
    ) {
      void dashboardStore.load(item.moduleId, item.pageId, dashboardStore.state.range);
    }
  }

  function handleAvailabilityAction(action: string): void {
    if (
      action === 'dashboard.action.configure_source' ||
      action === 'dashboard.action.import_data' ||
      action === 'dashboard.action.enable' ||
      action === 'dashboard.action.update_module'
    ) {
      appStore.select('settings', 'local');
    }
  }

  async function refresh(): Promise<void> {
    await appStore.refresh();
    const current = selectedItem;
    if (
      current &&
      current.pageId !== 'sources' &&
      current.pageId !== 'phases' &&
      current.pageId !== 'settings' &&
      dashboardStore.state.range
    ) {
      await dashboardStore.load(current.moduleId, current.pageId, dashboardStore.state.range);
    }
  }
</script>

<svelte:head>
  <title>{appState.bootstrap?.productName ?? message('app.title')}</title>
</svelte:head>

{#if appState.loading}
  <main class="shell"><section class="panel" aria-live="polite">{message('app.loading')}</section></main>
{:else if appState.error}
  <main class="shell">
    <section class="panel error" role="alert">
      <h1>{message('app.error')}</h1>
      <code>{appState.error.code}</code>
      <p>{appState.error.message}</p>
      <button type="button" onclick={() => void appStore.load()}>{message('app.retry')}</button>
    </section>
  </main>
{:else if appState.bootstrap}
  <main class="shell">
    <header class="masthead">
      <div>
        <p class="eyebrow">{message('app.title')}</p>
        <h1>{appState.bootstrap.productName}</h1>
      </div>
      <span class="locale" aria-label="Current locale">{locale}</span>
    </header>

    <StatusBanner status={appState.ingestionStatus} refreshing={appState.refreshing} />
    <div class="shell-actions">
      <button type="button" onclick={() => void refresh()} disabled={appState.refreshing}>
        {appState.refreshing ? message('app.refreshing') : message('app.refresh')}
      </button>
    </div>
    <Navigation items={appState.navigation?.items ?? []} selectedPageId={appState.selectedPageId} onSelect={openPage} />

    <section class="module-summary panel" aria-labelledby="module-summary-title">
      <div class="panel-heading">
        <h2 id="module-summary-title">{message('modules.title')}</h2>
        <span>{appState.bootstrap.modules.length}</span>
      </div>
      <ul>
        {#each appState.bootstrap.modules as module (module.id)}
          <li><span>{module.displayName ?? module.id}</span><span>{module.enabled ? 'Enabled' : 'Disabled'}</span></li>
        {/each}
      </ul>
    </section>

    {#if appState.selectedPageId === 'sources'}
      <SourcesQualityPage transport={props.transport} />
    {:else if appState.selectedPageId === 'phases'}
      <PhaseEventsPage transport={props.transport} />
    {:else if appState.selectedPageId === 'settings'}
      <SettingsPage transport={props.transport} />
    {:else if selectedItem}
      <DashboardPage
        item={selectedItem}
        {dashboardStore}
        {locale}
        onAvailabilityAction={handleAvailabilityAction}
      />
    {/if}
  </main>
{/if}
