<script lang="ts">
  import { onMount } from 'svelte';
  import { normalizeTransportError, type AppTransport } from '../transport';
  import type { PhaseEventInput, PhaseEventView } from '../types';
  import { message } from '../i18n';
  import { localCalendarDate } from '../local-date';

  export let transport: AppTransport;

  let events: PhaseEventView[] = [];
  let editingId: string | null = null;
  let eventType = '';
  let startDate = localCalendarDate();
  let endDate = localCalendarDate();
  let description = '';
  let excludeFromTdee = true;
  let error = '';
  let loading = true;
  let saving = false;
  let deleting = false;
  let pendingDelete: PhaseEventView | null = null;

  async function loadEvents(): Promise<void> {
    loading = true;
    error = '';
    try {
      events = await transport.listPhaseEvents();
    } catch (cause: unknown) {
      error = normalizeTransportError(cause).message;
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    void loadEvents();
  });

  function reset(): void {
    editingId = null;
    eventType = '';
    startDate = localCalendarDate();
    endDate = localCalendarDate();
    description = '';
    excludeFromTdee = true;
    error = '';
  }

  function edit(event: PhaseEventView): void {
    editingId = event.phaseEventId;
    eventType = event.eventType;
    startDate = event.startDate;
    endDate = event.endDate;
    description = event.description ?? '';
    excludeFromTdee = event.excludeFromTdee;
    error = '';
  }

  async function save(): Promise<void> {
    if (!eventType.trim() || startDate > endDate) {
      error = startDate > endDate ? message('phases.invalid_range') : message('phases.type');
      return;
    }
    const input: PhaseEventInput = {
      phaseEventId: editingId,
      eventType: eventType.trim(),
      startDate,
      endDate,
      description: description.trim() || null,
      excludeFromTdee,
    };
    saving = true;
    error = '';
    try {
      const saved = await transport.savePhaseEvent(input);
      events = editingId
        ? events.map((event) => event.phaseEventId === saved.phaseEventId ? saved : event)
        : [...events, saved];
      reset();
    } catch (cause: unknown) {
      error = normalizeTransportError(cause).message;
    } finally {
      saving = false;
    }
  }

  function requestDelete(event: PhaseEventView): void {
    pendingDelete = event;
    error = '';
  }

  function cancelDelete(): void {
    pendingDelete = null;
    error = '';
  }

  async function confirmDelete(): Promise<void> {
    const event = pendingDelete;
    if (!event) return;
    deleting = true;
    error = '';
    try {
      await transport.deletePhaseEvent(event.phaseEventId);
      events = events.filter((candidate) => candidate.phaseEventId !== event.phaseEventId);
      if (editingId === event.phaseEventId) reset();
      pendingDelete = null;
    } catch (cause: unknown) {
      error = normalizeTransportError(cause).message;
    } finally {
      deleting = false;
    }
  }
</script>

<section class="page panel" aria-labelledby="phases-title">
  <div class="page-heading">
    <div><p class="eyebrow">{message('navigation.phases')}</p><h2 id="phases-title">{message('phases.title')}</h2></div>
    <button type="button" on:click={reset}>{message('phases.add')}</button>
  </div>
  {#if pendingDelete}
    <div class="confirmation" role="dialog" aria-modal="true" aria-labelledby="phase-delete-title" aria-describedby="phase-delete-description">
      <h3 id="phase-delete-title">{message('phases.confirm_delete_title')}</h3>
      <p id="phase-delete-description">{message('phases.confirm_delete_message')} <strong>{pendingDelete.eventType}</strong></p>
      {#if error}<p role="alert" class="error-detail">{error}</p>{/if}
      <div class="form-actions">
        <button type="button" data-action="confirm-delete" disabled={deleting} on:click={() => void confirmDelete()}>
          {deleting ? message('dashboard.loading') : message('phases.confirm_delete_action')}
        </button>
        <button type="button" data-action="cancel-delete" disabled={deleting} on:click={cancelDelete}>
          {message('phases.cancel_delete')}
        </button>
      </div>
    </div>
  {/if}
  {#if loading}<p class="muted">{message('phases.loading')}</p>
  {:else if events.length === 0}<p class="muted">{message('phases.empty')}</p>{/if}
  <ul class="phase-list">
    {#each events as event (event.phaseEventId)}
      <li><div><strong>{event.eventType}</strong><span>{event.startDate} → {event.endDate}</span>{#if event.description}<span>{event.description}</span>{/if}</div><div class="phase-actions"><button type="button" on:click={() => edit(event)}>{message('phases.edit')}</button><button type="button" aria-label={`${message('phases.delete')}: ${event.eventType}`} on:click={() => requestDelete(event)}>{message('phases.delete')}</button></div></li>
    {/each}
  </ul>
  <form class="phase-form" aria-label={editingId ? message('phases.edit') : message('phases.add')} on:submit|preventDefault={() => void save()}>
    <label>{message('phases.type')}<input aria-label={message('phases.type')} bind:value={eventType} required /></label>
    <label>{message('phases.start')}<input aria-label={message('phases.start')} type="date" bind:value={startDate} required /></label>
    <label>{message('phases.end')}<input aria-label={message('phases.end')} type="date" bind:value={endDate} required /></label>
    <label>{message('phases.description')}<textarea aria-label={message('phases.description')} bind:value={description}></textarea></label>
    <label class="checkbox"><input type="checkbox" bind:checked={excludeFromTdee} /> {message('phases.exclude')}</label>
    {#if error && !pendingDelete}<p role="alert" class="error-detail">{error}</p>{/if}
    <div class="form-actions"><button type="submit" disabled={saving}>{saving ? message('dashboard.loading') : message('phases.save')}</button><button type="button" on:click={reset}>{message('phases.cancel')}</button></div>
  </form>
</section>
