import {
  BarChart,
  HeatmapChart,
  LineChart,
  ScatterChart,
} from "echarts/charts";
import {
  CalendarComponent,
  GridComponent,
  TitleComponent,
  TooltipComponent,
  VisualMapComponent,
} from "echarts/components";
import { init, use } from "echarts/core";
import type { ECharts, EChartsCoreOption } from "echarts/core";
import { SVGRenderer } from "echarts/renderers";
import type { ChartSeries } from "../../types";

type EChartsOption = EChartsCoreOption;

export type ChartKind = "line" | "bar" | "scatter" | "calendar_heatmap";

use([
  BarChart,
  HeatmapChart,
  LineChart,
  ScatterChart,
  CalendarComponent,
  GridComponent,
  TitleComponent,
  TooltipComponent,
  VisualMapComponent,
  SVGRenderer,
]);

export function chartOption(
  kind: ChartKind,
  series: ChartSeries[],
  stale: boolean,
): EChartsOption {
  const labels = [
    ...new Set(series.flatMap((item) => item.points.map(([label]) => label))),
  ];
  if (kind === "calendar_heatmap") {
    return {
      animation: false,
      calendar: {
        range: labels.length > 0 ? [labels[0], labels.at(-1)] : undefined,
      },
      visualMap: { min: 0, max: 1, calculable: false, show: false },
      series: series.map((item) => ({
        type: "heatmap",
        coordinateSystem: "calendar",
        data: item.points.filter(([, value]) => value !== null),
      })),
      ...(stale ? { aria: { decal: { show: true } } } : {}),
    };
  }
  const type = kind === "scatter" ? "scatter" : kind;
  return {
    animation: false,
    grid: { left: 36, right: 16, top: 16, bottom: 28, containLabel: true },
    tooltip: { trigger: "axis" },
    xAxis: { type: "category", data: labels },
    yAxis: { type: "value" },
    series: series.map((item) => ({
      name: item.name,
      type,
      connectNulls: false,
      data: labels.map(
        (label) => item.points.find(([point]) => point === label)?.[1] ?? null,
      ),
    })),
    ...(stale ? { aria: { decal: { show: true } } } : {}),
  };
}

export function mountChart(
  element: HTMLElement,
  kind: ChartKind,
  series: ChartSeries[],
  stale: boolean,
): ECharts | null {
  if (typeof ResizeObserver === "undefined") return null;
  const chart = init(element, undefined, { renderer: "svg" });
  chart.setOption(chartOption(kind, series, stale));
  return chart;
}

export function disposeChart(chart: ECharts | null): void {
  chart?.dispose();
}
