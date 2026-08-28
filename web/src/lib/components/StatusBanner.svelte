<script lang="ts">
  import type { IngestionStatus } from '../types';
  import { message } from '../i18n';

  export let status: IngestionStatus | null = null;
  export let refreshing = false;
  export let onOpenSettings: () => void = () => undefined;

  $: health = status?.health.state ?? 'working';
</script>

<section class={`status-banner status-${health}`} role="status" aria-live="polite">
  <strong>{message(`health.${health}`)}</strong>
  {#if status?.configured}
    <span>{status.health.workingJobs} active jobs · {status.health.attentionItems} attention items</span>
  {:else}
    <span>{message('settings.workspace_unconfigured')}</span>
  {/if}
  {#if status && Object.keys(status.health.failureCodeCounts).length > 0}
    <div class="status-detail">
      <strong>{message('health.import_error')}</strong>
      <ul>
        {#each Object.entries(status.health.failureCodeCounts) as [code, count] (code)}
          <li><code>{code}</code> ({count})</li>
        {/each}
      </ul>
    </div>
  {/if}
  {#if status && status.pendingModuleUpdates.length > 0}
    <div class="status-detail">
      <strong>{message('health.module_updates_required')} {status.pendingModuleUpdates.join(', ')}</strong>
      <button type="button" data-action="open-settings" on:click={onOpenSettings}>
        {message('health.open_settings')}
      </button>
    </div>
  {/if}
  {#if refreshing}<span>{message('app.refreshing')}</span>{/if}
</section>
