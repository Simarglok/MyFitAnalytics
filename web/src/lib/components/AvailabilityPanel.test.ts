import { mount, unmount } from "svelte";
import { describe, expect, it } from "vitest";
import AvailabilityPanel from "./AvailabilityPanel.svelte";
import type { AvailabilityView } from "../types";

const insufficientCoverage: AvailabilityView = {
  state: "insufficient_coverage",
  reasonKey: "dashboard.insufficient_coverage",
  requiredCapabilities: ["body.weight"],
  requiredDependencies: [],
  action: "dashboard.action.import_data",
};

describe("AvailabilityPanel", () => {
  it("renders the computed insufficient-coverage action as a localized control", () => {
    const target = document.createElement("div");
    document.body.append(target);
    const actions: string[] = [];
    const app = mount(AvailabilityPanel, {
      target,
      props: {
        availability: insufficientCoverage,
        onAction: (action: string) => actions.push(action),
      },
    });

    expect(
      target.querySelector('[data-availability-state="insufficient_coverage"]'),
    ).not.toBeNull();
    expect(target.textContent).toContain("More data is needed for this range");
    const action = target.querySelector<HTMLButtonElement>(
      '[data-availability-action="dashboard.action.import_data"]',
    );
    expect(action).not.toBeNull();
    expect(action?.textContent).toContain("Import data");
    action?.click();
    expect(actions).toEqual(["dashboard.action.import_data"]);
    unmount(app);
  });

  it("renders guidance instead of a control for an unknown action", () => {
    const target = document.createElement("div");
    document.body.append(target);
    const app = mount(AvailabilityPanel, {
      target,
      props: {
        availability: {
          ...insufficientCoverage,
          action: "dashboard.action.unknown",
        },
      },
    });

    expect(
      target.querySelector(
        '[data-availability-guidance="dashboard.action.unknown"]',
      ),
    ).not.toBeNull();
    expect(target.querySelector("button[data-availability-action]")).toBeNull();
    expect(target.textContent).toContain("Next step");
    unmount(app);
  });
});
