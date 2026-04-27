import { test, expect } from "@playwright/test";

test.describe("gap drill-down /gap/{verdict}/{decision}/{condition}", () => {
  test("real gap renders tutorial + test stub @smoke", async ({ page }) => {
    // parser_dispatch decision 2 condition 2 is a known gap (see manifest).
    const resp = await page.goto("/gap/parser_dispatch/2/2");
    expect(resp?.status()).toBe(200);
    await expect(page.locator("h1")).toContainText("Gap analysis");
    await expect(page.locator(".status-gap")).toBeVisible();
    await expect(page.locator("body")).toContainText("What you need");
    await expect(page.locator("body")).toContainText("Suggested test stub");
    await expect(page.locator("pre.stub")).toContainText("#[test]");
    await expect(page.locator("pre.stub")).toContainText("closes_gap_d2_c2");
  });

  test("proved condition shows already-proved early-out", async ({ page }) => {
    await page.goto("/gap/triangle/0/0");
    await expect(page.locator(".status-proved")).toBeVisible();
    await expect(page.locator("body")).toContainText("Already proved");
    await expect(page.locator("pre.stub")).toHaveCount(0);
  });

  test("dead condition shows reachability hint", async ({ page }) => {
    // parser_dispatch d0 has dead conditions; pick c1 from the known data.
    await page.goto("/gap/parser_dispatch/0/1");
    await expect(page.locator("body")).toContainText("dead");
    await expect(page.locator("body")).toContainText("never reached");
  });

  test("condition list links from /decision to /gap", async ({ page }) => {
    await page.goto("/decision/parser_dispatch/2");
    const gapLinks = page.locator("a.gap-link");
    expect(await gapLinks.count()).toBeGreaterThan(0);
    const href = await gapLinks.first().getAttribute("href");
    expect(href).toMatch(/^\/gap\/parser_dispatch\/2\/\d+$/);
  });

  test("/gap with unknown condition index returns 404", async ({ page }) => {
    const resp = await page.goto("/gap/triangle/0/999");
    expect(resp?.status()).toBe(404);
  });
});
