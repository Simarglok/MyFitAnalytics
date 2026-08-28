<script lang="ts">
  import AvailabilityPanel from './AvailabilityPanel.svelte';
  import BarChart from './charts/BarChart.svelte';
  import CalendarHeatmap from './charts/CalendarHeatmap.svelte';
  import LineChart from './charts/LineChart.svelte';
  import ScatterChart from './charts/ScatterChart.svelte';
  import { message } from '../i18n';
  import { formatNumber } from '../i18n/format';
  import type {
    AvailabilityView,
    ChartSeries,
    DashboardBlock,
    DashboardChart,
    DashboardDocument,
    DashboardCardPresentation,
    DashboardSummaryValue,
    DashboardTable,
  } from '../types';

  export let document: unknown;
  export let availability: AvailabilityView | null = null;
  export let stale = false;
  export let locale = 'en-US';

  type SafeDocument = DashboardDocument;

  function decodeDocument(value: unknown): SafeDocument | null {
    if (!isRecord(value) || !Array.isArray(value.blocks)) {
      return null;
    }
    const titleKey = typeof value.titleKey === 'string'
      ? value.titleKey
      : typeof value.title_key === 'string'
        ? value.title_key
        : null;
    if (titleKey === null) return null;
    const blocks = value.blocks.map(decodeBlock);
    return blocks.every((block): block is DashboardBlock => block !== null)
      ? { titleKey, blocks }
      : null;
  }

  function decodeOutput(value: unknown):
    | { kind: 'document'; document: SafeDocument }
    | { kind: 'module_error'; error: { code: string; messageKey: string } }
    | null {
    const document = decodeDocument(value);
    if (document) return { kind: 'document', document };
    if (!isRecord(value) || typeof value.code !== 'string') return null;
    const messageKey = typeof value.messageKey === 'string'
      ? value.messageKey
      : typeof value.message_key === 'string'
        ? value.message_key
        : null;
    return messageKey === null
      ? null
      : { kind: 'module_error', error: { code: value.code, messageKey } };
  }

  function decodeBlock(value: unknown): DashboardBlock | null {
    if (!isRecord(value) || typeof value.type !== 'string' || !isRecord(value.value)) return null;
    switch (value.type) {
      case 'card': {
        if (typeof value.value.key !== 'string' || typeof value.value.label !== 'string') {
          return null;
        }
        const presentation = decodePresentation(value.value.presentation);
        if (presentation === null) return null;
        return {
          type: 'card',
          value: {
            key: value.value.key,
            label: value.value.label,
            value: value.value.value,
            ...(presentation === undefined ? {} : { presentation }),
          },
        };
      }
      case 'table':
        return decodeTable(value.value);
      case 'status_panel':
        return decodeStatus(value.value);
      case 'chart':
        return decodeChart(value.value);
      default:
        return null;
    }
  }

  function decodeTable(value: Record<string, unknown>): DashboardBlock | null {
    if (
      typeof value.key !== 'string' ||
      !Array.isArray(value.columns) ||
      !value.columns.every((column) => typeof column === 'string') ||
      !Array.isArray(value.rows) ||
      !value.rows.every((row) => Array.isArray(row))
    ) {
      return null;
    }
    return {
      type: 'table',
      value: { key: value.key, columns: value.columns, rows: value.rows as unknown[][] },
    };
  }

  function decodePresentation(value: unknown): DashboardCardPresentation | undefined | null {
    if (value === undefined || value === null) return undefined;
    if (!isRecord(value)) return null;
    const summaryKey = typeof value.summaryKey === 'string'
      ? value.summaryKey
      : typeof value.summary_key === 'string'
        ? value.summary_key
        : null;
    if (summaryKey === null) return null;
    const summaryValue = value.summaryValue ?? value.summary_value;
    if (summaryValue !== undefined && summaryValue !== null && !isSummaryValue(summaryValue)) {
      return null;
    }
    return {
      summaryKey,
      ...(summaryValue === undefined
        ? {}
        : { summaryValue: summaryValue as DashboardSummaryValue | null }),
    };
  }

  function isSummaryValue(value: unknown): value is DashboardSummaryValue {
    return (
      typeof value === 'string' ||
      typeof value === 'boolean' ||
      (typeof value === 'number' && Number.isFinite(value))
    );
  }

  function decodeStatus(value: Record<string, unknown>): DashboardBlock | null {
    const messageKey = typeof value.messageKey === 'string'
      ? value.messageKey
      : typeof value.message_key === 'string'
        ? value.message_key
        : null;
    if (typeof value.key !== 'string' || messageKey === null || !isRecord(value.state)) {
      return null;
    }
    const state = typeof value.state.state === 'string' ? value.state.state : value.state.type;
    return typeof state === 'string' && state in availabilityLabels
      ? {
          type: 'status_panel',
          value: {
            key: value.key,
            state: {
              state: state as AvailabilityView['state'],
              reasonKey: messageKey,
              requiredCapabilities: [],
              requiredDependencies: [],
            },
            messageKey,
          },
        }
      : null;
  }

  function decodeChart(value: Record<string, unknown>): DashboardBlock | null {
    if (
      typeof value.key !== 'string' ||
      !isChartType(value.chartType ?? value.chart_type) ||
      !Array.isArray(value.series)
    ) {
      return null;
    }
    const series: ChartSeries[] = [];
    for (const candidate of value.series) {
      if (!isRecord(candidate) || typeof candidate.name !== 'string' || !Array.isArray(candidate.points)) {
        return null;
      }
      const points: [string, number | null][] = [];
      for (const point of candidate.points) {
        if (!Array.isArray(point) || typeof point[0] !== 'string') return null;
        if (point[1] !== null && (typeof point[1] !== 'number' || !Number.isFinite(point[1]))) return null;
        points.push([point[0], point[1] as number | null]);
      }
      series.push({ name: candidate.name, points });
    }
    return {
      type: 'chart',
      value: { key: value.key, chartType: (value.chartType ?? value.chart_type) as DashboardChart['chartType'], series },
    };
  }

  function isRecord(value: unknown): value is Record<string, unknown> {
    return typeof value === 'object' && value !== null && !Array.isArray(value);
  }

  function isChartType(value: unknown): value is DashboardChart['chartType'] {
    return value === 'line' || value === 'bar' || value === 'scatter' || value === 'calendar_heatmap';
  }

  const availabilityLabels: Record<string, true> = {
    missing_capability: true,
    missing_dependency: true,
    incompatible_contract: true,
    waiting_for_data: true,
    insufficient_coverage: true,
    ready: true,
    disabled_by_user: true,
  };

  $: safeOutput = decodeOutput(document);

  function cardValue(value: unknown, presentation?: DashboardCardPresentation): string {
    if (presentation) {
      const summary = message(presentation.summaryKey, presentation.summaryKey);
      if (presentation.summaryValue === undefined || presentation.summaryValue === null) {
        return summary;
      }
      const renderedValue = typeof presentation.summaryValue === 'number'
        ? formatNumber(presentation.summaryValue, locale)
        : String(presentation.summaryValue);
      return `${summary}: ${renderedValue}`;
    }
    if (value === null || value === undefined) return message('dashboard.card_empty');
    if (typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean') {
      return String(value);
    }
    return message('dashboard.card_empty');
  }

  function chartTitle(key: string): string {
    return message(key, key);
  }
</script>

{#if !safeOutput}
  <section class="panel error" role="alert">
    <h2>{message('dashboard.content_unavailable')}</h2>
  </section>
{:else if safeOutput.kind === 'module_error'}
  <section class="panel error" role="alert" data-module-error={safeOutput.error.code}>
    <h2>{message(safeOutput.error.messageKey, safeOutput.error.code)}</h2>
  </section>
{:else}
  <section class="dashboard-document" aria-labelledby="dashboard-document-title">
    <h2 id="dashboard-document-title">{message(safeOutput.document.titleKey, safeOutput.document.titleKey)}</h2>
    {#if availability}
      <AvailabilityPanel {availability} />
    {/if}
    {#if safeOutput.document.blocks.length === 0}
      <p class="muted">{message('dashboard.no_points')}</p>
    {/if}
    {#each safeOutput.document.blocks as block (block.value.key)}
      {#if block.type === 'card'}
        <article class="dashboard-card" data-block-type="card">
          <span>{message(block.value.label, block.value.label)}</span>
          <strong>{cardValue(block.value.value, block.value.presentation)}</strong>
        </article>
      {:else if block.type === 'table'}
        <div class="dashboard-table-wrap" data-block-type="table">
          {#if block.value.rows.length === 0}
            <p class="muted">{message('dashboard.table_empty')}</p>
          {:else}
            <table>
              <thead><tr>{#each block.value.columns as column (column)}<th scope="col">{column}</th>{/each}</tr></thead>
              <tbody>
                {#each block.value.rows as row, rowIndex (rowIndex)}
                  <tr>{#each row as cell, cellIndex (`${rowIndex}-${cellIndex}`)}<td>{typeof cell === 'string' || typeof cell === 'number' ? cell : cardValue(cell)}</td>{/each}</tr>
                {/each}
              </tbody>
            </table>
          {/if}
        </div>
      {:else if block.type === 'status_panel'}
        <aside class="dashboard-status" data-block-type="status_panel" role="status">
          {message(block.value.messageKey, block.value.messageKey)}
        </aside>
      {:else if block.value.chartType === 'line'}
        <LineChart title={chartTitle(block.value.key)} series={block.value.series} {stale} />
      {:else if block.value.chartType === 'bar'}
        <BarChart title={chartTitle(block.value.key)} series={block.value.series} {stale} />
      {:else if block.value.chartType === 'scatter'}
        <ScatterChart title={chartTitle(block.value.key)} series={block.value.series} {stale} />
      {:else}
        <CalendarHeatmap title={chartTitle(block.value.key)} series={block.value.series} {stale} />
      {/if}
    {/each}
  </section>
{/if}
