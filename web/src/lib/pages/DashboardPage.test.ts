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

const initialRange = { start: "2026-01-01", end: "2026-01-31" };
const rebasedRange = { start: "2026-01-04", end: "2026-02-03" };

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

  it("synchronizes clean date controls when the applied store range changes", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const transport = new MockTransport({ dashboards: { overview: page } });
    const dashboardStore = createDashboardStore(transport, initialRange);
    const app = mount(DashboardPage, {
      target,
      props: { item, dashboardStore },
    });

    await dashboardStore.load("base", "overview");
    await dashboardStore.load("base", "overview", rebasedRange);

    await vi.waitFor(() => {
      expect(
        target.querySelector<HTMLInputElement>('[aria-label="Range start"]')
          ?.value,
      ).toBe(rebasedRange.start);
      expect(
        target.querySelector<HTMLInputElement>('[aria-label="Range end"]')
          ?.value,
      ).toBe(rebasedRange.end);
    });
    unmount(app);
  });

  it("does not overwrite a dirty date control when the store range changes", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const transport = new MockTransport({ dashboards: { overview: page } });
    const dashboardStore = createDashboardStore(transport, initialRange);
    const app = mount(DashboardPage, {
      target,
      props: { item, dashboardStore },
    });

    await dashboardStore.load("base", "overview");
    const start = target.querySelector<HTMLInputElement>(
      '[aria-label="Range start"]',
    );
    if (!start) throw new Error("range start input was not rendered");
    start.value = "2025-12-01";
    start.dispatchEvent(new Event("input", { bubbles: true }));

    await dashboardStore.load("base", "overview", rebasedRange);

    await vi.waitFor(() => expect(start.value).toBe("2025-12-01"));
    unmount(app);
  });

  it("keeps an applied custom range visible when the store range changes", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const transport = new MockTransport({ dashboards: { overview: page } });
    const dashboardStore = createDashboardStore(transport, initialRange);
    const app = mount(DashboardPage, {
      target,
      props: { item, dashboardStore },
    });

    await dashboardStore.load("base", "overview");
    await dashboardStore.load("base", "overview", {
      start: "2025-12-01",
      end: "2025-12-31",
    });

    await vi.waitFor(() => {
      expect(
        target.querySelector<HTMLInputElement>('[aria-label="Range start"]')
          ?.value,
      ).toBe("2025-12-01");
      expect(
        target.querySelector<HTMLInputElement>('[aria-label="Range end"]')
          ?.value,
      ).toBe("2025-12-31");
    });
    unmount(app);
  });
});
