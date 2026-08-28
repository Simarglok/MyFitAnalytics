<script lang="ts">
  import type { AvailabilityView } from '../types';
  import { message } from '../i18n';

  export let availability: AvailabilityView | null = null;
  export let onAction: ((action: string) => void) | undefined = undefined;

  const labels: Record<string, string> = {
    missing_capability: 'dashboard.state.missing_capability',
    missing_dependency: 'dashboard.state.missing_dependency',
    incompatible_contract: 'dashboard.state.incompatible_contract',
    waiting_for_data: 'dashboard.state.waiting_for_data',
    insufficient_coverage: 'dashboard.state.insufficient_coverage',
    ready: 'dashboard.state.ready',
    disabled_by_user: 'dashboard.state.disabled_by_user',
  };

  const actionLabels: Record<string, string> = {
    'dashboard.action.configure_source': 'Configure source',
    'dashboard.action.import_data': 'Import data',
    'dashboard.action.enable': 'Enable module',
    'dashboard.action.update_module': 'Update module',
  };
  const actionable = new Set(Object.keys(actionLabels));

  $: stateLabel = availability ? message(labels[availability.state] ?? availability.state) : '';
  $: reasonLabel = availability ? message(availability.reasonKey, availability.reasonKey) : '';
  $: actionLabel = availability?.action
    ? message(availability.action, actionLabels[availability.action] ?? availability.action)
    : '';
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
    {#if availability.action}
      {#if actionable.has(availability.action) && onAction}
        <button
          type="button"
          data-availability-action={availability.action}
          on:click={() => onAction?.(availability.action as string)}
        >{actionLabel}</button>
      {:else}
        <span data-availability-guidance={availability.action}>
          {message('dashboard.action.guidance')}: {actionLabel}
        </span>
      {/if}
    {/if}
  </aside>
{/if}
