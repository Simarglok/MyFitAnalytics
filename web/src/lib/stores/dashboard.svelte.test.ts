import { describe, expect, it } from "vitest";
import { MockTransport } from "../mock-transport";
import { createDashboardStore } from "./dashboard.svelte";

const backendInitialRange = { start: "2026-03-16", end: "2026-04-15" };

describe("dashboard store initialization", () => {
  it("uses the backend navigation range instead of a fixture date", () => {
    const store = createDashboardStore(
      new MockTransport(),
      backendInitialRange,
    );

    expect(store.state.range).toEqual(backendInitialRange);
  });
});
