import { mount, unmount } from "svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import PhaseEventsPage from "./PhaseEventsPage.svelte";
import { MockTransport } from "../mock-transport";
import type { PhaseEventView } from "../types";

const persistedEvent: PhaseEventView = {
  phaseEventId: "phase-1",
  eventType: "synthetic-test",
  startDate: "2026-02-10",
  endDate: "2026-02-12",
  description: "persisted phase",
  excludeFromTdee: true,
};

afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

describe("PhaseEventsPage", () => {
  it("loads committed events on mount and again after re-entry", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const transport = new MockTransport({ phaseEvents: [persistedEvent] });

    const first = mount(PhaseEventsPage, { target, props: { transport } });
    await vi.waitFor(() =>
      expect(target.textContent).toContain(persistedEvent.eventType),
    );
    unmount(first);

    const second = mount(PhaseEventsPage, { target, props: { transport } });
    await vi.waitFor(() =>
      expect(target.textContent).toContain(persistedEvent.description),
    );
    expect(
      transport.calls.filter((call) => call === "listPhaseEvents"),
    ).toHaveLength(2);
    unmount(second);
  });

  it("uses the current local calendar date for a new form", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 3, 15, 23, 30));
    const target = document.createElement("div");
    document.body.append(target);

    const app = mount(PhaseEventsPage, {
      target,
      props: { transport: new MockTransport() },
    });

    await vi.waitFor(() =>
      expect(target.textContent).toContain("Phase events"),
    );
    const dates = [
      ...target.querySelectorAll<HTMLInputElement>('input[type="date"]'),
    ];
    expect(dates.map((input) => input.value)).toEqual([
      "2026-04-15",
      "2026-04-15",
    ]);
    unmount(app);
  });

  it("opens an in-app confirmation naming the event without deleting", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const transport = new MockTransport({ phaseEvents: [persistedEvent] });
    const nativeConfirm = vi.spyOn(window, "confirm");
    const app = mount(PhaseEventsPage, { target, props: { transport } });

    await vi.waitFor(() =>
      expect(target.textContent).toContain(persistedEvent.eventType),
    );
    target
      .querySelector<HTMLButtonElement>(
        'button[aria-label="Delete phase event: synthetic-test"]',
      )
      ?.click();

    const dialog = await vi.waitFor(() => {
      const element = target.querySelector<HTMLElement>('[role="dialog"]');
      expect(element).toBeTruthy();
      return element;
    });
    expect(dialog?.textContent).toContain(persistedEvent.eventType);
    expect(dialog?.textContent).toContain("Confirm delete");
    expect(dialog?.textContent).toContain("Cancel");
    expect(nativeConfirm).not.toHaveBeenCalled();
    expect(transport.calls).not.toContain("deletePhaseEvent:phase-1");
    unmount(app);
  });

  it("cancels an in-app deletion without changing the event", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const transport = new MockTransport({ phaseEvents: [persistedEvent] });
    const app = mount(PhaseEventsPage, { target, props: { transport } });

    await vi.waitFor(() =>
      expect(target.textContent).toContain(persistedEvent.eventType),
    );
    target
      .querySelector<HTMLButtonElement>(
        'button[aria-label="Delete phase event: synthetic-test"]',
      )
      ?.click();
    await vi.waitFor(() =>
      expect(target.querySelector('[role="dialog"]')).toBeTruthy(),
    );
    target
      .querySelector<HTMLButtonElement>('button[data-action="cancel-delete"]')
      ?.click();

    await vi.waitFor(() =>
      expect(target.querySelector('[role="dialog"]')).toBeNull(),
    );
    expect(target.textContent).toContain(persistedEvent.eventType);
    expect(transport.calls).not.toContain("deletePhaseEvent:phase-1");
    unmount(app);
  });

  it("updates after edit and deletes only after confirmation", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const transport = new MockTransport({ phaseEvents: [persistedEvent] });
    const app = mount(PhaseEventsPage, { target, props: { transport } });

    await vi.waitFor(() =>
      expect(target.textContent).toContain(persistedEvent.eventType),
    );
    const edit = [...target.querySelectorAll<HTMLButtonElement>("button")].find(
      (button) => button.textContent?.includes("Edit phase event"),
    );
    expect(edit).toBeTruthy();
    edit?.click();
    const type = target.querySelector<HTMLInputElement>(
      'input[aria-label="Event type"]',
    );
    expect(type).toBeTruthy();
    type!.value = "maintenance";
    type!.dispatchEvent(new Event("input", { bubbles: true }));
    const save = [...target.querySelectorAll<HTMLButtonElement>("button")].find(
      (button) => button.textContent?.includes("Save phase event"),
    );
    save?.click();
    await vi.waitFor(() =>
      expect(transport.calls).toContain("savePhaseEvent:phase-1"),
    );
    await vi.waitFor(() => expect(target.textContent).toContain("maintenance"));

    const deleteButton = target.querySelector<HTMLButtonElement>(
      'button[aria-label="Delete phase event: maintenance"]',
    );
    expect(deleteButton).toBeTruthy();
    deleteButton?.click();
    await vi.waitFor(() =>
      expect(target.querySelector('[role="dialog"]')).toBeTruthy(),
    );
    expect(transport.calls).not.toContain("deletePhaseEvent:phase-1");
    target
      .querySelector<HTMLButtonElement>('button[data-action="confirm-delete"]')
      ?.click();
    await vi.waitFor(() =>
      expect(transport.calls).toContain("deletePhaseEvent:phase-1"),
    );
    await vi.waitFor(() =>
      expect(target.textContent).toContain("No phase events recorded."),
    );
    expect(target.textContent).not.toContain("maintenance");
    expect(target.querySelector("form")?.getAttribute("aria-label")).toBe(
      "Add phase event",
    );
    unmount(app);
  });

  it("keeps the event visible and reports a typed delete error", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const transport = new MockTransport({
      phaseEvents: [persistedEvent],
      deletePhaseEventError: {
        code: "phase_event_delete_failed",
        message: "delete failed",
      },
    });
    const app = mount(PhaseEventsPage, { target, props: { transport } });

    await vi.waitFor(() =>
      expect(target.textContent).toContain(persistedEvent.eventType),
    );
    const deleteButton = target.querySelector<HTMLButtonElement>(
      'button[aria-label="Delete phase event: synthetic-test"]',
    );
    deleteButton?.click();
    await vi.waitFor(() =>
      expect(target.querySelector('[role="dialog"]')).toBeTruthy(),
    );
    expect(transport.calls).not.toContain("deletePhaseEvent:phase-1");
    target
      .querySelector<HTMLButtonElement>('button[data-action="confirm-delete"]')
      ?.click();
    await vi.waitFor(() =>
      expect(target.textContent).toContain("delete failed"),
    );
    expect(target.textContent).toContain(persistedEvent.eventType);
    expect(transport.calls).not.toContain("deletePhaseEvent:phase-1");
    expect(target.querySelector('[role="dialog"]')).toBeTruthy();
    unmount(app);
  });

  it("keeps the committed event unchanged and reports a typed save error", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const transport = new MockTransport({
      phaseEvents: [persistedEvent],
      savePhaseEventError: {
        code: "phase_event_save_failed",
        message: "save failed",
      },
    });
    const app = mount(PhaseEventsPage, { target, props: { transport } });

    await vi.waitFor(() =>
      expect(target.textContent).toContain(persistedEvent.eventType),
    );
    const edit = [...target.querySelectorAll<HTMLButtonElement>("button")].find(
      (button) => button.textContent?.includes("Edit phase event"),
    );
    edit?.click();
    const type = target.querySelector<HTMLInputElement>(
      'input[aria-label="Event type"]',
    );
    type!.value = "failed-edit";
    type!.dispatchEvent(new Event("input", { bubbles: true }));
    [...target.querySelectorAll<HTMLButtonElement>("button")]
      .find((button) => button.textContent?.includes("Save phase event"))
      ?.click();

    await vi.waitFor(() => expect(target.textContent).toContain("save failed"));
    expect(target.querySelector("strong")?.textContent).toBe(
      persistedEvent.eventType,
    );
    expect(transport.calls).not.toContain("savePhaseEvent:phase-1");
    unmount(app);
  });
});
