import { test, expect } from "@playwright/test";
import {
  getJson,
  SummaryPayload,
  VerdictListEntry,
} from "./helpers";

test.describe("api /api/v1/*", () => {
  test("/api/v1/summary returns expected keys", async ({ request }) => {
    const summary = await getJson<SummaryPayload>(request, "/api/v1/summary");
    expect(summary).toHaveProperty("decisions_total");
    expect(summary).toHaveProperty("decisions_full_mcdc");
    expect(summary).toHaveProperty("conditions_proved");
    expect(summary).toHaveProperty("verdicts");
  });

  test("decisions_total is a positive integer", async ({ request }) => {
    const summary = await getJson<SummaryPayload>(request, "/api/v1/summary");
    expect(Number.isInteger(summary.decisions_total)).toBeTruthy();
    expect(summary.decisions_total).toBeGreaterThan(0);
  });

  test("verdicts count matches dashboard table", async ({ request, page }) => {
    const summary = await getJson<SummaryPayload>(request, "/api/v1/summary");
    await page.goto("/");
    // Dashboard has one data row per verdict plus a TOTAL row at the bottom.
    const totalRows = await page.locator("table tbody tr").count();
    const dataRows = await page
      .locator("table tbody tr")
      .filter({ hasNot: page.locator(".total-row") })
      .count();
    const verdictRowCount = dataRows > 0 ? dataRows : totalRows - 1;
    expect(verdictRowCount).toBe(summary.verdicts);
  });

  test("/api/v1/verdicts is an array, length matches summary.verdicts", async ({
    request,
  }) => {
    const summary = await getJson<SummaryPayload>(request, "/api/v1/summary");
    const list = await getJson<VerdictListEntry[]>(request, "/api/v1/verdicts");
    expect(Array.isArray(list)).toBeTruthy();
    expect(list.length).toBe(summary.verdicts);
  });

  test("/api/v1/verdict/triangle decisions[0].status === full_mcdc", async ({
    request,
  }) => {
    const verdict = await getJson<{
      decisions: { id: number; status: string }[];
    }>(request, "/api/v1/verdict/triangle");
    expect(verdict.decisions.length).toBeGreaterThan(0);
    expect(verdict.decisions[0].status).toBe("full_mcdc");
  });

  test("/api/v1/decision/triangle/0 has truth_table with 4 rows", async ({
    request,
  }) => {
    const decision = await getJson<{
      id: number;
      truth_table: unknown[];
    }>(request, "/api/v1/decision/triangle/0");
    expect(Array.isArray(decision.truth_table)).toBeTruthy();
    expect(decision.truth_table.length).toBe(4);
  });

  test("/api/v1/verdict/nonexistent returns 404", async ({ request }) => {
    const res = await request.get("/api/v1/verdict/nonexistent");
    expect(res.status()).toBe(404);
  });
});
