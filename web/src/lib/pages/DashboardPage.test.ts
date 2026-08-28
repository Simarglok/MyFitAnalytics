import { mount, unmount } from "svelte";
import { describe, expect, it, vi } from "vitest";
import DashboardPage from "./DashboardPage.svelte";
import { MockTransport } from "../mock-transport";
import { createDashboardStore } from "../stores/dashboard.svelte";
import type { NavigationItemView } from "../types";

const item: NavigationItemView = {
  id: "base:overview",
  pageId: "overview",
  titleKey: "base.overview.title",
  moduleId: "base",
  availability: {
    state: "ready",
    reasonKey: "dashboard.ready",
    requiredCapabilities: [],
    requiredDependencies: [],
    action: null,
  },
};

const page = {
  moduleId: "base",
  pageId: "overview",
  titleKey: "base.overview.title",
  document: {
    titleKey: "base.overview.title",
    blocks: [
      {
        type: "card" as const,
        value: {
          key: "weight",
          label: "base.overview.body_weight",
          value: 72.5,
        },
      },
    ],
  },
  availability: item.availability,
  coverage: { expectedDays: 30, observedDays: 30, ratio: 1, sufficient: true },
  freshness: {
    latestObservationDate: "2026-01-31",
    generatedAt: "2026-02-01T00:00:00Z",
  },
};

describe("DashboardPage", () => {
  it("renders a dashboard response that arrives after the page mounts", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const transport = new MockTransport({ dashboards: { overview: page } });
    const dashboardStore = createDashboardStore(transport, {
      start: "2026-01-01",
      end: "2026-01-31",
    });
    const app = mount(DashboardPage, {
      target,
      props: { item, dashboardStore },
    });

    await dashboardStore.load("base", "overview");
    await vi.waitFor(() => expect(target.textContent).toContain("72.5"));
    expect(target.textContent).toContain("Overview");
    unmount(app);
  });

  it("explains when the visible dashboard is stale after a data change", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const transport = new MockTransport({ dashboards: { overview: page } });
    const dashboardStore = createDashboardStore(transport, {
      start: "2026-01-01",
      end: "2026-01-31",
    });
    const app = mount(DashboardPage, {
      target,
      props: { item, dashboardStore },
    });

    await dashboardStore.load("base", "overview");
    dashboardStore.markStale();
    await vi.waitFor(() =>
      expect(target.textContent).toContain("Dashboard data may be stale"),
    );
    unmount(app);
  });
});
