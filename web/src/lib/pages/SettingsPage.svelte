<script lang="ts">
  import { onMount } from 'svelte';
  import { normalizeTransportError } from '../transport';
  import type { AppTransport } from '../transport';
  import type { ModuleCatalogEntry, WorkspaceView } from '../types';
  import { message } from '../i18n';

  export let transport: AppTransport;

  let catalog: ModuleCatalogEntry[] = [];
  let workspace: WorkspaceView | null = null;
  let errorCode = '';
  let errorMessage = '';
  let activeProviders: Record<string, string> = {};
  let pendingUninstall: string | null = null;
  let loading = true;

  const localizedErrors: Record<string, string> = {
    workspace_required: message('settings.error.workspace_required'),
    module_must_be_disabled: message('settings.error.module_must_be_disabled'),
    incompatible_app_version: message('settings.error.incompatible_app_version'),
    incompatible_source_api: message('settings.error.incompatible_app_version'),
    incompatible_dashboard_api: message('settings.error.incompatible_app_version'),
    incompatible_package_format: message('settings.error.incompatible_app_version'),
    package_io_error: message('settings.error.package_io_error'),
    module_update_unavailable: message('settings.error.module_update_unavailable'),
  };

  onMount(() => {
    void reload();
  });

  async function reload(): Promise<void> {
    loading = true;
    try {
      const catalogPromise = transport.listModuleCatalog?.() ?? Promise.resolve([]);
      const workspacePromise = transport.getWorkspaceView?.() ?? Promise.resolve(null);
      const [loadedCatalog, loadedWorkspace] = await Promise.all([catalogPromise, workspacePromise]);
      catalog = loadedCatalog.slice().sort((left, right) => {
        const categoryDifference = categoryOrder(left.module.moduleType) - categoryOrder(right.module.moduleType);
        return categoryDifference || left.module.id.localeCompare(right.module.id);
      });
      workspace = loadedWorkspace;
      errorCode = '';
      errorMessage = '';
    } catch (error: unknown) {
      showError(error);
    } finally {
      loading = false;
    }
  }

  async function run(action: () => Promise<unknown>): Promise<void> {
    try {
      errorCode = '';
      errorMessage = '';
      await action();
      await reload();
    } catch (error: unknown) {
      showError(error);
    }
  }

  function showError(error: unknown): void {
    const normalized = normalizeTransportError(error);
    errorCode = normalized.code;
    errorMessage = localizedErrors[normalized.code] ?? message('settings.error.module_action_failed');
  }

  async function chooseWorkspace(): Promise<void> {
    const chosen = await transport.chooseWorkspaceRoot?.();
    if (chosen) workspace = chosen;
  }

  async function installPackage(): Promise<void> {
    await transport.chooseAndInstallModule?.();
  }

  async function toggleModule(moduleId: string, enabled: boolean): Promise<void> {
    await transport.setModuleEnabled?.(moduleId, enabled);
  }

  async function updateModule(moduleId: string): Promise<void> {
    await transport.updateModule?.(moduleId);
  }

  function requestUninstall(moduleId: string): void {
    pendingUninstall = moduleId;
  }

  async function confirmUninstall(): Promise<void> {
    const moduleId = pendingUninstall;
    pendingUninstall = null;
    if (!moduleId) return;
    await transport.uninstallModule?.(moduleId);
  }

  async function chooseInbox(moduleId: string): Promise<void> {
    const chosen = await transport.chooseSourceInbox?.(moduleId);
    if (chosen) workspace = chosen;
  }

  async function selectProvider(capability: string, moduleId: string): Promise<void> {
    const selection = transport.selectProvider
      ? await transport.selectProvider(capability, moduleId)
      : await transport.selectModuleProvider?.(capability, moduleId);
    if (selection) activeProviders = selection.activeProviders;
  }

  function sourcePath(moduleId: string): string | null {
    return workspace?.sourcePaths.find((path) => path.moduleId === moduleId)?.inboxPath ?? null;
  }

  function stateLabel(state: ModuleCatalogEntry['installState']): string {
    return message(`settings.state.${state}`);
  }

  function categoryOrder(moduleType: ModuleCatalogEntry['module']['moduleType']): number {
    return moduleType === 'source' ? 0 : moduleType === 'dashboard' ? 1 : 2;
  }

  function categoryLabel(moduleType: ModuleCatalogEntry['module']['moduleType']): string {
    return message(`settings.group.${moduleType}`);
  }

  function moduleName(entry: ModuleCatalogEntry): string {
    if (entry.module.displayName) return entry.module.displayName;
    if (entry.module.id === 'hevy') return 'Hevy';
    if (entry.module.id === 'mynetdiary') return 'MyNetDiary';
    return entry.module.id;
  }
</script>

<section class="settings panel" aria-labelledby="settings-title">
  <div class="panel-heading">
    <div>
      <p class="eyebrow">{message('settings.eyebrow')}</p>
      <h2 id="settings-title">{message('settings.sources')}</h2>
    </div>
    <button type="button" data-action="install-package" on:click={() => void run(installPackage)}>
      {message('settings.install_package')}
    </button>
  </div>

  <div class="settings-paths">
    <div>
      <strong>{message('settings.workspace')}</strong>
      <span>{workspace?.workspaceRoot || message('settings.workspace_unconfigured')}</span>
    </div>
    <button type="button" data-action="choose-workspace" on:click={() => void run(chooseWorkspace)}>
      {message('settings.choose_workspace')}
    </button>
  </div>

  <label class="locale-select">
    {message('settings.locale')}
    <select aria-label={message('settings.locale')}>
      <option value="en">{message('settings.locale_english')}</option>
    </select>
  </label>

  {#if loading}
    <p aria-live="polite">{message('settings.loading')}</p>
  {:else if catalog.length === 0}
    <p class="muted">{message('settings.empty')}</p>
  {:else}
    <h3>{message('settings.sources')}</h3>
    <div class="module-catalog">
      {#each catalog as entry, index (entry.module.id)}
        {#if index === 0 || catalog[index - 1].module.moduleType !== entry.module.moduleType}
          <h4 class="module-group-heading">{categoryLabel(entry.module.moduleType)}</h4>
        {/if}
        <article class="module-card" data-module-id={entry.module.id}>
          <div class="module-card-heading">
            <div>
              <h3>{moduleName(entry)}</h3>
              <span>{entry.origin} · {entry.module.version}</span>
            </div>
            <strong class="module-status">{stateLabel(entry.installState)}</strong>
          </div>

          {#if entry.errorCode}
            <p class="error-detail" role="alert">
              <code>{entry.errorCode}</code>
              {localizedErrors[entry.errorCode] ?? message('settings.error.module_action_failed')}
            </p>
          {/if}

          {#if entry.availableVersion}
            <p>{message('settings.available_version')} {entry.availableVersion}</p>
          {/if}

          {#if sourcePath(entry.module.id)}
            <p class="path-detail">{message('settings.inbox')} {sourcePath(entry.module.id)}</p>
          {/if}

          <div class="module-actions">
            {#if entry.installState === 'available'}
              <button type="button" data-action="install" data-module-id={entry.module.id} on:click={() => void run(() => updateModule(entry.module.id))}>
                {message('settings.install')}
              </button>
            {:else if entry.installState === 'update'}
              <button type="button" data-action="update" data-module-id={entry.module.id} on:click={() => void run(() => updateModule(entry.module.id))}>
                {message('settings.update')}
              </button>
            {/if}

            {#if entry.module.enabled}
              <button type="button" data-action="disable" data-module-id={entry.module.id} on:click={() => void run(() => toggleModule(entry.module.id, false))}>
                {message('settings.disable')}
              </button>
            {:else if entry.installState !== 'available' && entry.installState !== 'error' && entry.installState !== 'incompatible'}
              <button type="button" data-action="enable" data-module-id={entry.module.id} on:click={() => void run(() => toggleModule(entry.module.id, true))}>
                {message('settings.enable')}
              </button>
              <button type="button" data-action="uninstall" data-module-id={entry.module.id} on:click={() => requestUninstall(entry.module.id)}>
                {message('settings.uninstall')}
              </button>
            {/if}

            {#if entry.module.moduleType === 'source' && entry.installState !== 'available'}
              <button type="button" data-action="choose-inbox" data-module-id={entry.module.id} on:click={() => void run(() => chooseInbox(entry.module.id))}>
                {message('settings.choose_inbox')}
              </button>
            {/if}

            {#each entry.module.providedCapabilities ?? [] as capability (capability)}
              <button type="button" data-action="provider" data-module-id={entry.module.id} on:click={() => void run(() => selectProvider(capability, entry.module.id))}>
                {message('settings.use_for')} {capability}
              </button>
              {#if activeProviders[capability] === entry.module.id}
                <span>{message('settings.active_provider')}</span>
              {/if}
            {/each}
          </div>
        </article>
      {/each}
    </div>
  {/if}

  {#if pendingUninstall}
    <div class="confirmation" role="dialog" aria-modal="true">
      <p>{message('settings.confirm_uninstall')}</p>
      <button type="button" data-action="confirm-uninstall" on:click={() => void run(confirmUninstall)}>
        {message('settings.confirm')}
      </button>
      <button type="button" data-action="cancel-uninstall" on:click={() => (pendingUninstall = null)}>
        {message('settings.cancel')}
      </button>
    </div>
  {/if}

  {#if errorMessage}
    <div class="error" role="alert">
      <code>{errorCode}</code>
      <p>{errorMessage}</p>
    </div>
  {/if}
</section>
