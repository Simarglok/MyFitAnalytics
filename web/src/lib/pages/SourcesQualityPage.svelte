<script lang="ts">
  import { onMount } from 'svelte';
  import { normalizeTransportError, type AppTransport, type TransportError } from '../transport';
  import type { QualityItem } from '../types';
  import { message } from '../i18n';

  export let transport: AppTransport;

  let items: QualityItem[] = [];
  let loading = true;
  let listError: TransportError | null = null;
  let retryError: TransportError | null = null;
  let retrying = new Set<string>();

  onMount(() => {
    void reload();
  });

  async function reload(): Promise<void> {
    loading = true;
    listError = null;
    retryError = null;
    try {
      items = await transport.listQualityItems();
    } catch (cause: unknown) {
      listError = normalizeTransportError(cause);
    } finally {
      loading = false;
    }
  }

  async function retry(item: QualityItem): Promise<void> {
    retrying = new Set(retrying).add(item.id);
    retryError = null;
    try {
      if (item.assetId) await transport.retryAsset(item.assetId);
      await reload();
    } catch (cause: unknown) {
      retryError = normalizeTransportError(cause);
    } finally {
      const next = new Set(retrying);
      next.delete(item.id);
      retrying = next;
    }
  }
</script>

<section class="page panel" aria-labelledby="quality-title">
  <div class="page-heading">
    <div>
      <p class="eyebrow">{message('navigation.sources')}</p>
      <h2 id="quality-title">{message('quality.title')}</h2>
    </div>
    <button type="button" on:click={() => void reload()}>{message('app.refresh')}</button>
  </div>
  {#if loading}
    <p aria-live="polite">{message('dashboard.loading')}</p>
  {:else}
    {#if listError}
      <div role="alert" class="error" data-list-error><code>{listError.code}</code><p>{message('quality.error')}</p></div>
    {/if}
    {#if retryError}
      <div role="alert" class="error" data-retry-error><code>{retryError.code}</code><p>{message('quality.retry_error')}</p></div>
    {/if}
    {#if items.length === 0 && !listError}
      <p class="muted">{message('quality.empty')}</p>
    {:else if items.length > 0}
    <div class="quality-table-wrap">
      <table>
        <thead><tr><th scope="col">{message('quality.severity')}</th><th scope="col">{message('quality.message')}</th><th scope="col">{message('quality.status')}</th><th scope="col">Action</th></tr></thead>
        <tbody>
          {#each items as item (item.id)}
            <tr>
              <td>{item.severity}</td><td>{#if item.code}<code data-quality-code={item.code}>{item.code}</code>{:else}{item.message}{/if}</td><td>{item.status}</td>
              <td>{#if item.assetId}<button type="button" on:click={() => void retry(item)} disabled={retrying.has(item.id)}>{retrying.has(item.id) ? message('quality.retrying') : message('quality.retry')}</button>{/if}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    {/if}
  {/if}
</section>
