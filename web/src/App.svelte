<script lang="ts">
  import { onMount } from 'svelte';
  import { normalizeTransportError } from './lib/transport';
  import type { AppTransport } from './lib/transport';
  import type { BootstrapState, IngestionStatus, ModuleView } from './lib/types';
  import { message } from './lib/i18n';

  export let transport: AppTransport;

  let bootstrap: BootstrapState | null = null;
  let modules: ModuleView[] = [];
  let ingestionStatus: IngestionStatus | null = null;
  let loading = true;
  let errorCode = '';
  let errorMessage = '';

  onMount(() => {
    let active = true;
    const loadView = async () => {
      const [state, status] = await Promise.all([
        transport.getBootstrapState(),
        transport.getIngestionStatus(),
      ]);
      if (!active) return;
      bootstrap = state;
      modules = state.modules;
      ingestionStatus = status;
      loading = false;
    };

    loadView().catch((error: unknown) => {
        if (!active) return;
        const normalized = normalizeTransportError(error);
        errorCode = normalized.code;
        errorMessage = normalized.message;
        loading = false;
      });

    let unlisten: (() => void) | undefined;
    transport
      .subscribeDataChanged(() => {
        loadView().catch(() => undefined);
      })
      .then((cleanup) => {
        if (!active) {
          cleanup();
          return;
        }
        unlisten = cleanup;
      })
      .catch(() => undefined);

    return () => {
      active = false;
      unlisten?.();
    };
  });
</script>

<svelte:head>
  <title>{bootstrap?.productName ?? message('app.title')}</title>
</svelte:head>

<main class="shell">
  <header class="masthead">
    <div>
      <p class="eyebrow">Personal health workspace</p>
      <h1>{bootstrap?.productName ?? message('app.title')}</h1>
    </div>
    {#if bootstrap}
      <span class="locale" aria-label="Current locale">{bootstrap.locale}</span>
    {/if}
  </header>

  {#if loading}
    <section class="panel" aria-live="polite">
      <p>{message('app.loading')}</p>
    </section>
  {:else if errorMessage}
    <section class="panel error" role="alert">
      <h2>{message('app.error')}</h2>
      <code>{errorCode}</code>
      <p>{errorMessage}</p>
    </section>
  {:else if bootstrap}
    {#if ingestionStatus}
      <section class="panel" aria-label="Ingestion status">
        <strong>Storage</strong>
        <span>{ingestionStatus.health.state}</span>
        {#if ingestionStatus.recoveryMode !== 'normal'}
          <span>{ingestionStatus.recoveryMode}</span>
        {/if}
      </section>
    {/if}
    <section class="panel" aria-labelledby="modules-title">
      <div class="panel-heading">
        <h2 id="modules-title">{message('modules.title')}</h2>
        <span>{modules.length}</span>
      </div>
      {#if modules.length === 0}
        <p class="muted">No modules installed yet.</p>
      {:else}
        <ul>
          {#each modules as module (module.id)}
            <li>
              <div>
                <strong>{module.id}</strong>
                <span>{module.localizationNamespace}</span>
              </div>
              <span class:disabled={!module.enabled} class="module-status">
                {module.enabled ? 'Enabled' : 'Disabled'} · {module.version}
              </span>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  {/if}
</main>
