import { test, expect } from "@playwright/test";

test.describe("verdict /verdict/{name}", () => {
  test("/verdict/triangle returns 200 @smoke", async ({ page }) => {
    const response = await page.goto("/verdict/triangle");
    expect(response?.status()).toBe(200);
  });

  test("page contains a Decisions heading", async ({ page }) => {
    await page.goto("/verdict/triangle");
    const heading = page.locator("h2", { hasText: /Decisions/i });
    await expect(heading.first()).toBeVisible();
  });

  test("decisions are linked to /decision/triangle/...", async ({ page }) => {
    await page.goto("/verdict/triangle");
    const links = page.locator('a[href^="/decision/triangle/"]');
    const count = await links.count();
    expect(count).toBeGreaterThan(0);

    const firstHref = await links.first().getAttribute("href");
    expect(firstHref).toMatch(/^\/decision\/triangle\/\d+$/);
  });

  test("/verdict/does-not-exist returns 404", async ({ page }) => {
    const response = await page.goto("/verdict/does-not-exist");
    expect(response?.status()).toBe(404);
  });

  test("sidebar overview link returns to dashboard", async ({ page }) => {
    await page.goto("/verdict/triangle");
    await page.locator('nav.sidebar a[href="/"]', { hasText: "Overview" }).first().click();
    await expect(page).toHaveURL(/\/$/);
    await expect(page.locator("h1", { hasText: /Compliance overview/i })).toBeVisible();
  });
});
