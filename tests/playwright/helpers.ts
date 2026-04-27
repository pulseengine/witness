import { Page, APIRequestContext, expect } from "@playwright/test";

/** Count rows in the first <tbody> on the page. */
export async function countTableRows(page: Page): Promise<number> {
  return page.locator("table tbody tr").first().locator("xpath=..").locator("tr").count();
}

/**
 * Count rows across every <tbody> on the page.
 * Most witness-viz pages have a single table; the dashboard has one and the
 * decision view has the truth-table.
 */
export async function countAllTableRows(page: Page): Promise<number> {
  return page.locator("table tbody tr").count();
}

/** Fetch JSON from an API path and return it as `T`. Asserts 200 OK. */
export async function getJson<T>(
  request: APIRequestContext,
  path: string,
): Promise<T> {
  const res = await request.get(path);
  expect(res.ok(), `GET ${path} -> ${res.status()}`).toBeTruthy();
  return (await res.json()) as T;
}

/** Fetch JSON expecting a specific status code; returns parsed body or null. */
export async function getJsonStatus(
  request: APIRequestContext,
  path: string,
  expected: number,
): Promise<unknown> {
  const res = await request.get(path);
  expect(res.status()).toBe(expected);
  const body = await res.text();
  if (!body) return null;
  try {
    return JSON.parse(body);
  } catch {
    return body;
  }
}

/** Shape of the /api/v1/summary payload. */
export interface SummaryPayload {
  verdicts: number;
  branches: number;
  decisions_total: number;
  decisions_full_mcdc: number;
  conditions_total: number;
  conditions_proved: number;
  conditions_gap: number;
  conditions_dead: number;
}

/** Shape of an entry from /api/v1/verdicts. */
export interface VerdictListEntry {
  name: string;
  branches: number;
  decisions_total: number;
  decisions_full_mcdc: number;
  conditions_total: number;
  conditions_proved: number;
  conditions_gap: number;
  conditions_dead: number;
  status: string;
}
