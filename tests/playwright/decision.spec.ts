import { test, expect } from "@playwright/test";

test.describe("decision /decision/{verdict}/{id}", () => {
  test("/decision/triangle/0 returns 200 @smoke", async ({ page }) => {
    const response = await page.goto("/decision/triangle/0");
    expect(response?.status()).toBe(200);
  });

  test("truth-table widget rendered with row column", async ({ page }) => {
    await page.goto("/decision/triangle/0");
    const table = page.locator("table.truth-table");
    await expect(table).toBeVisible();
    await expect(table.locator("thead th", { hasText: /^row$/ })).toBeVisible();
  });

  test("every condition column header matches c<idx> br <id>", async ({ page }) => {
    await page.goto("/decision/triangle/0");
    // First and last <th> are "row" and "outcome"; the middle ones are
    // condition headers rendered as `c{idx} <span class="br">br {id}</span>`.
    const headers = page.locator("table.truth-table thead th");
    const total = await headers.count();
    expect(total).toBeGreaterThan(2);
    for (let i = 1; i < total - 1; i++) {
      const txt = (await headers.nth(i).textContent())?.replace(/\s+/g, " ").trim() ?? "";
      expect(txt).toMatch(/^c\d+\s+br\s+\d+$/);
    }
  });

  test("status indicator status-full_mcdc shown for triangle/0", async ({ page }) => {
    await page.goto("/decision/triangle/0");
    await expect(page.locator(".status.status-full_mcdc").first()).toBeVisible();
  });

  test("Independent-effect pairs heading exists", async ({ page }) => {
    await page.goto("/decision/triangle/0");
    await expect(
      page.locator("h2", { hasText: /Independent-effect pairs/i }),
    ).toBeVisible();
  });

  test("at least one condition is shown as proved", async ({ page }) => {
    await page.goto("/decision/triangle/0");
    const proved = page.locator("ul.conditions li.cond-proved");
    expect(await proved.count()).toBeGreaterThan(0);
  });

  test("/decision/triangle/999 returns 404", async ({ page }) => {
    const response = await page.goto("/decision/triangle/999");
    expect(response?.status()).toBe(404);
  });

  test("partial-MCDC decision shows row-gap or gap condition", async ({ page }) => {
    // parser_dispatch decision 0 is partial but its gap conditions are all
    // dead — pick a decision that genuinely has a gap-status condition so
    // the row-gap class can fire. Decision 2 in the fixture is partial with
    // one gap condition.
    const response = await page.goto("/decision/parser_dispatch/2");
    expect(response?.status()).toBe(200);

    const rowGap = page.locator("table.truth-table tbody tr.row-gap");
    const gapCond = page.locator("ul.conditions li.cond-gap");
    const rowGapCount = await rowGap.count();
    const gapCondCount = await gapCond.count();
    expect(rowGapCount + gapCondCount).toBeGreaterThan(0);
  });
});
