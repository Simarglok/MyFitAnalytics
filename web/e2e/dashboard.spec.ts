import { test, expect } from "@playwright/test";

type MockConfig = {
  dashboardAvailability?: "ready" | "disabled_by_user";
  health?: "healthy" | "working" | "attention" | "blocked";
};

async function openMockApp(
  page: import("@playwright/test").Page,
  config: MockConfig = {},
): Promise<void> {
  await page.addInitScript((value) => {
    (
      window as Window & {
        __MFA_MOCK__?: boolean;
        __MFA_MOCK_CONFIG__?: MockConfig;
      }
    ).__MFA_MOCK__ = true;
    (
      window as Window & { __MFA_MOCK_CONFIG__?: MockConfig }
    ).__MFA_MOCK_CONFIG__ = value;
  }, config);
  await page.goto("/");
}

test.describe("dashboard through deterministic mock transport", () => {
  test("loads the base dashboard with a populated chart and null gap", async ({
    page,
  }) => {
    await openMockApp(page);
    await expect(
      page.getByRole("heading", { name: "MyFitAnalytics" }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Overview" }),
    ).toHaveAttribute("aria-current", "page");
    await expect(page.locator('[data-chart-type="line"]')).toBeVisible();
    await expect(page.locator('[data-point-gap="true"]')).toHaveCount(1);
  });

  test("changes the visible dashboard range without leaving the shell", async ({
    page,
  }) => {
    await openMockApp(page);
    await page.getByLabel("Range start").fill("2026-02-01");
    await page.getByLabel("Range end").fill("2026-02-28");
    await page.getByRole("button", { name: "Apply range" }).click();
    await expect(page.getByText("Feb 28, 2026")).toBeVisible();
    await expect(page.locator('[data-chart-type="line"]')).toBeVisible();
  });

  test("keeps a disabled graph visible with an explanation", async ({
    page,
  }) => {
    await openMockApp(page, { dashboardAvailability: "disabled_by_user" });
    await expect(page.getByText("This dashboard is disabled")).toBeVisible();
    await expect(page.locator('[data-chart-type="line"]')).toBeVisible();
  });

  test("exposes Settings, quality retry, and phase-event entry points", async ({
    page,
  }) => {
    await openMockApp(page);
    await page.getByRole("button", { name: "Sources & quality" }).click();
    await expect(page.getByText("Settings", { exact: true })).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Install Module Package…" }),
    ).toBeVisible();
    await expect(
      page.getByText("One deterministic quality issue"),
    ).toBeVisible();
    await page.getByRole("button", { name: "Retry import" }).click();
    await page.getByRole("button", { name: "Phase events" }).click();
    await page.getByLabel("Event type").fill("deload");
    await page.getByLabel("Start date").fill("2026-02-10");
    await page.getByLabel("End date").fill("2026-02-12");
    await page.getByRole("button", { name: "Save phase event" }).click();
    await expect(page.getByText("deload")).toBeVisible();
  });

  test("shows the blocked recovery banner without a modal", async ({
    page,
  }) => {
    await openMockApp(page, { health: "blocked" });
    await expect(
      page.getByRole("status").filter({ hasText: "Blocked" }),
    ).toBeVisible();
    await expect(page.getByRole("button", { name: "Overview" })).toBeVisible();
  });
});
