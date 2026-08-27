<script lang="ts">
  import { afterUpdate, onMount } from 'svelte';
  import type { ECharts } from 'echarts/core';
  import type { ChartSeries } from '../../types';
  import { disposeChart, mountChart } from './chart-utils';

  export let title = 'Bar chart';
  export let series: ChartSeries[] = [];
  export let stale = false;

  let host: HTMLDivElement;
  let chart: ECharts | null = null;

  function renderChart(): void {
    disposeChart(chart);
    chart = host ? mountChart(host, 'bar', series, stale) : null;
  }

  onMount(() => {
    renderChart();
    return () => disposeChart(chart);
  });

  afterUpdate(renderChart);
</script>

<section class="chart-frame" data-chart-type="bar" aria-label={title} role="img">
  <div class="chart-canvas" bind:this={host} aria-hidden="true"></div>
  <ul class="chart-points" aria-label={`${title} values`}>
    {#each series as item (item.name)}
      {#each item.points as [label, value] (label)}
        <li data-point-gap={value === null ? 'true' : undefined}>
          <span>{label}</span>
          <span>{value === null ? '—' : value}</span>
        </li>
      {/each}
    {/each}
  </ul>
</section>
