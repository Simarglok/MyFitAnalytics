<script lang="ts">
  import type { NavigationItemView } from '../types';
  import { message } from '../i18n';

  export let items: NavigationItemView[] = [];
  export let selectedPageId = '';
  export let onSelect: (item: NavigationItemView) => void = () => undefined;

  function label(item: NavigationItemView): string {
    const page = item.pageId === 'sources' ? 'navigation.sources' : `navigation.${item.pageId}`;
    return message(page, item.titleKey);
  }
</script>

<nav class="navigation panel" aria-label={message('navigation.title')}>
  <h2>{message('navigation.title')}</h2>
  <div class="navigation-list">
    {#each items as item (item.id)}
      <button
        type="button"
        class:active={item.pageId === selectedPageId}
        aria-current={item.pageId === selectedPageId ? 'page' : undefined}
        aria-label={label(item)}
        on:click={() => onSelect(item)}
      >
        <span>{label(item)}</span>
        <small data-availability-state={item.availability.state}>{message(`dashboard.state.${item.availability.state}`)}</small>
      </button>
    {/each}
    <button
      type="button"
      class:active={selectedPageId === 'phases'}
      aria-current={selectedPageId === 'phases' ? 'page' : undefined}
      on:click={() => onSelect({
        id: 'local:phases', pageId: 'phases', titleKey: 'navigation.phases', moduleId: 'local',
        availability: { state: 'ready', reasonKey: 'dashboard.ready', requiredCapabilities: [], requiredDependencies: [] },
      })}
    >
      <span>{message('navigation.phases')}</span>
    </button>
  </div>
</nav>
