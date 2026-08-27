<script lang="ts">
  import type { AvailabilityView } from '../types';
  import { message } from '../i18n';

  export let availability: AvailabilityView | null = null;

  const labels: Record<string, string> = {
    missing_capability: 'dashboard.state.missing_capability',
    missing_dependency: 'dashboard.state.missing_dependency',
    incompatible_contract: 'dashboard.state.incompatible_contract',
    waiting_for_data: 'dashboard.state.waiting_for_data',
    insufficient_coverage: 'dashboard.state.insufficient_coverage',
    ready: 'dashboard.state.ready',
    disabled_by_user: 'dashboard.state.disabled_by_user',
  };

  $: stateLabel = availability ? message(labels[availability.state] ?? availability.state) : '';
  $: reasonLabel = availability ? message(availability.reasonKey, availability.reasonKey) : '';
</script>

{#if availability}
  <aside class="availability-panel" role="status" aria-live="polite">
    <strong>{message('dashboard.availability')}</strong>
    <span data-availability-reason>{reasonLabel}</span>
    <span data-availability-state={availability.state}>{stateLabel}</span>
    {#if availability.requiredCapabilities.length > 0}
      <span>{message('dashboard.required_capabilities')}: {availability.requiredCapabilities.join(', ')}</span>
    {/if}
    {#if availability.requiredDependencies.length > 0}
      <span>{message('dashboard.required_dependencies')}: {availability.requiredDependencies.join(', ')}</span>
    {/if}
  </aside>
{/if}
