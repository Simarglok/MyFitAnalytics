import { describe, expect, it } from "vitest";
import { localCalendarDate } from "./local-date";

describe("localCalendarDate", () => {
  it("formats the local calendar components without UTC truncation", () => {
    const localDate = new Date(2026, 3, 15, 0, 30, 0, 0);
    expect(localCalendarDate(localDate)).toBe("2026-04-15");
  });
});
