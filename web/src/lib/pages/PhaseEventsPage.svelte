<script lang="ts">
  import type { AppTransport } from '../transport';
  import type { PhaseEventInput, PhaseEventView } from '../types';
  import { message } from '../i18n';

  export let transport: AppTransport;
  export let initialEvents: PhaseEventView[] = [];

  let events = [...initialEvents];
  let editingId: string | null = null;
  let eventType = '';
  let startDate = '2026-01-01';
  let endDate = '2026-01-01';
  let description = '';
  let excludeFromTdee = true;
  let error = '';
  let saving = false;

  function reset(): void {
    editingId = null;
    eventType = '';
    startDate = '2026-01-01';
    endDate = '2026-01-01';
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
      const saved = await transport.savePhaseEvent?.(input);
      if (saved) {
        events = editingId
          ? events.map((event) => event.phaseEventId === saved.phaseEventId ? saved : event)
          : [...events, saved];
      }
      reset();
    } catch (cause: unknown) {
      error = cause instanceof Error ? cause.message : message('dashboard.error');
    } finally {
      saving = false;
    }
  }
</script>

<section class="page panel" aria-labelledby="phases-title">
  <div class="page-heading">
    <div><p class="eyebrow">{message('navigation.phases')}</p><h2 id="phases-title">{message('phases.title')}</h2></div>
    <button type="button" on:click={reset}>{message('phases.add')}</button>
  </div>
  {#if events.length === 0}<p class="muted">{message('phases.empty')}</p>{/if}
  <ul class="phase-list">
    {#each events as event (event.phaseEventId)}
      <li><div><strong>{event.eventType}</strong><span>{event.startDate} → {event.endDate}</span>{#if event.description}<span>{event.description}</span>{/if}</div><button type="button" on:click={() => edit(event)}>{message('phases.edit')}</button></li>
    {/each}
  </ul>
  <form class="phase-form" aria-label={editingId ? message('phases.edit') : message('phases.add')} on:submit|preventDefault={() => void save()}>
    <label>{message('phases.type')}<input aria-label={message('phases.type')} bind:value={eventType} required /></label>
    <label>{message('phases.start')}<input aria-label={message('phases.start')} type="date" bind:value={startDate} required /></label>
    <label>{message('phases.end')}<input aria-label={message('phases.end')} type="date" bind:value={endDate} required /></label>
    <label>{message('phases.description')}<textarea aria-label={message('phases.description')} bind:value={description}></textarea></label>
    <label class="checkbox"><input type="checkbox" bind:checked={excludeFromTdee} /> {message('phases.exclude')}</label>
    {#if error}<p role="alert" class="error-detail">{error}</p>{/if}
    <div class="form-actions"><button type="submit" disabled={saving}>{saving ? message('dashboard.loading') : message('phases.save')}</button><button type="button" on:click={reset}>{message('phases.cancel')}</button></div>
  </form>
</section>
