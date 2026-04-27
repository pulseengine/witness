import { test, expect } from "@playwright/test";

test.describe("dashboard /", () => {
  test("title contains Overview and witness-viz", async ({ page }) => {
    await page.goto("/");
    await expect(page).toHaveTitle(/Overview/);
    await expect(page).toHaveTitle(/witness-viz/);
  });

  test("headline cards present with non-zero counts", async ({ page }) => {
    await page.goto("/");
    const cards = page.locator(".cards .card");
    await expect(cards).not.toHaveCount(0);

    // Decisions, Full MC/DC, Conditions proved are the first three cards.
    const labels = await cards.locator(".label").allTextContents();
    expect(labels).toEqual(
      expect.arrayContaining(["Decisions", "Full MC/DC", "Conditions proved"]),
    );

    for (const label of ["Decisions", "Full MC/DC", "Conditions proved"]) {
      const card = cards.filter({ has: page.locator(`.label:text-is("${label}")`) });
      const numText = (await card.locator(".num").textContent()) ?? "0";
      const num = Number(numText.trim());
      expect(num, `${label} card count`).toBeGreaterThan(0);
    }
  });

  test("verdict table has at least one row with non-empty proved cell", async ({ page }) => {
    await page.goto("/");
    const dataRows = page.locator("table tbody tr").filter({ hasNot: page.locator(".total-row") });
    const count = await dataRows.count();
    expect(count).toBeGreaterThan(0);

    const provedCells = page.locator("table tbody tr td.proved");
    const provedCount = await provedCells.count();
    expect(provedCount).toBeGreaterThan(0);

    let nonEmpty = 0;
    for (let i = 0; i < provedCount; i++) {
      const txt = (await provedCells.nth(i).textContent())?.trim() ?? "";
      if (txt.length > 0 && txt !== "0") nonEmpty++;
    }
    expect(nonEmpty, "at least one verdict has proved > 0").toBeGreaterThan(0);
  });

  test("each verdict row links to /verdict/{name}", async ({ page }) => {
    await page.goto("/");
    const verdictLinks = page.locator('table tbody tr a[href^="/verdict/"]');
    const count = await verdictLinks.count();
    expect(count).toBeGreaterThan(0);

    const firstHref = await verdictLinks.first().getAttribute("href");
    expect(firstHref).toMatch(/^\/verdict\/[^/]+$/);
  });

  test("sidebar nav links exist", async ({ page }) => {
    await page.goto("/");
    const sidebar = page.locator("nav.sidebar");
    await expect(sidebar.locator('a[href="/"]', { hasText: "Overview" })).toBeVisible();
    await expect(
      sidebar.locator('a[href="/api/v1/summary"]', { hasText: "JSON summary" }),
    ).toBeVisible();
  });
});
