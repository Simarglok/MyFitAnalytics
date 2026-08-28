import { mount, unmount } from "svelte";
import { describe, expect, it } from "vitest";
import DashboardRenderer from "./DashboardRenderer.svelte";
import type { AvailabilityView } from "../types";

const ready: AvailabilityView = {
  state: "ready",
  reasonKey: "dashboard.ready",
  requiredCapabilities: [],
  requiredDependencies: [],
};

const documentWithEveryAllowedNode = {
  titleKey: "base.overview.title",
  blocks: [
    {
      type: "card",
      value: { key: "weight", label: "Weight", value: { value: 72.4 } },
    },
    {
      type: "table",
      value: {
        key: "sources",
        columns: ["Source", "Status"],
        rows: [["Hevy", "Ready"]],
      },
    },
    {
      type: "status_panel",
      value: {
        key: "quality",
        state: ready,
        messageKey: "base.overview.quality_ready",
      },
    },
    {
      type: "chart",
      value: {
        key: "trend",
        chartType: "line",
        series: [
          {
            name: "Weight",
            points: [
              ["2026-01-01", 72.4],
              ["2026-01-02", null],
            ],
          },
        ],
      },
    },
  ],
};

function render(
  dashboardDocument: unknown,
  availability: AvailabilityView = ready,
) {
  const target = document.createElement("div");
  document.body.append(target);
  const app = mount(DashboardRenderer, {
    target,
    props: { document: dashboardDocument, availability },
  });
  return { target, app };
}

describe("DashboardRenderer", () => {
  it("renders every allowed declarative node and preserves null chart gaps", () => {
    const { target, app } = render(documentWithEveryAllowedNode);

    expect(target.textContent).toContain("Weight");
    expect(target.textContent).toContain("Hevy");
    expect(target.textContent).toContain("Data is ready");
    expect(target.querySelector('[data-chart-type="line"]')).not.toBeNull();
    expect(target.querySelector('[data-point-gap="true"]')).not.toBeNull();
    expect(target.querySelectorAll("script")).toHaveLength(0);
    unmount(app);
  });

  it("decodes actual Rust-shaped dashboard JSON without losing content", () => {
    const { target, app } = render({
      title_key: "base.body.title",
      blocks: [
        {
          type: "card",
          value: { key: "weight", label: "Weight", value: 82.5 },
        },
        {
          type: "chart",
          value: {
            key: "trend",
            chart_type: "line",
            series: [{ name: "Weight", points: [["2026-01-01", 82.5]] }],
          },
        },
      ],
    });

    expect(target.textContent).toContain("Weight");
    expect(target.textContent).toContain("82.5");
    expect(target.textContent).not.toContain("Dashboard content unavailable");
    expect(target.querySelector('[data-chart-type="line"]')).not.toBeNull();
    unmount(app);
  });

  it("renders a Rust-shaped structured card through its reviewed presentation", () => {
    const { target, app } = render({
      title_key: "base.nutrition.title",
      blocks: [
        {
          type: "card",
          value: {
            key: "nutrition.calories",
            label: "base.nutrition.calories",
            value: {
              available: true,
              value: {
                days: [{ local_date: "2026-01-01", calories_kcal: 2400 }],
              },
            },
            presentation: {
              summary_key: "base.nutrition.calories",
              summary_value: 2400,
            },
          },
        },
      ],
    });

    expect(target.textContent).toContain("Calories: 2,400");
    expect(target.textContent).not.toContain("[object Object]");
    expect(target.textContent).not.toContain('{"available"');
    unmount(app);
  });

  it("uses a safe localized fallback instead of rendering an unreviewed object", () => {
    const { target, app } = render({
      title_key: "base.body.title",
      blocks: [
        {
          type: "card",
          value: {
            key: "body.raw_weight",
            label: "base.body.raw_weight",
            value: { available: true, value: { private_metric: 82.5 } },
          },
        },
      ],
    });

    expect(target.textContent).toContain("Not available");
    expect(target.textContent).not.toContain("private_metric");
    expect(target.textContent).not.toContain("{");
    unmount(app);
  });

  it("decodes the Rust-shaped typed module error as a safe error panel", () => {
    const { target, app } = render({
      code: "module_invoke_error",
      messageKey: "dashboard.module_error.module_invoke_error",
    });

    expect(
      target.querySelector('[data-module-error="module_invoke_error"]'),
    ).not.toBeNull();
    expect(target.querySelector('[role="alert"]')).not.toBeNull();
    expect(target.querySelectorAll("script")).toHaveLength(0);
    expect(target.textContent).not.toContain("Dashboard content unavailable");
    unmount(app);
  });

  it.each([
    "missing_capability",
    "missing_dependency",
    "incompatible_contract",
    "waiting_for_data",
    "insufficient_coverage",
    "ready",
    "disabled_by_user",
  ])("renders the %s availability state as an explanatory panel", (state) => {
    const { target, app } = render(
      { titleKey: "base.body.title", blocks: [] },
      {
        state: state as AvailabilityView["state"],
        reasonKey: "dashboard.availability",
        requiredCapabilities: ["body.weight"],
        requiredDependencies: ["base"],
      },
    );

    expect(target.querySelector('[role="status"]')).not.toBeNull();
    expect(target.textContent).toContain(state.replaceAll("_", " "));
    unmount(app);
  });

  it("rejects unknown nodes with a safe error panel and no executable content", () => {
    const { target, app } = render({
      titleKey: "module.untrusted.title",
      blocks: [{ type: "script", value: { source: "javascript:alert(1)" } }],
    });

    expect(target.querySelector('[role="alert"]')).not.toBeNull();
    expect(target.textContent).toContain("Dashboard content unavailable");
    expect(target.innerHTML).not.toContain("javascript:alert");
    expect(target.querySelectorAll("script")).toHaveLength(0);
    unmount(app);
  });
});
