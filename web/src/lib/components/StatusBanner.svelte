<script lang="ts">
  import type { IngestionStatus } from '../types';
  import { message } from '../i18n';

  export let status: IngestionStatus | null = null;
  export let refreshing = false;

  $: health = status?.health.state ?? 'working';
</script>

<section class={`status-banner status-${health}`} role="status" aria-live="polite">
  <strong>{message(`health.${health}`)}</strong>
  {#if status?.configured}
    <span>{status.health.workingJobs} active jobs · {status.health.attentionItems} attention items</span>
  {:else}
    <span>{message('settings.workspace_unconfigured')}</span>
  {/if}
  {#if refreshing}<span>{message('app.refreshing')}</span>{/if}
</section>
