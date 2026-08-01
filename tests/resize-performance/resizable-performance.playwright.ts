import { expect, test, type Locator, type Page } from "@playwright/test";

const maximumScriptedDragMs = 5_000;
const maximumFrameGapMs = 250;

test("keeps idle, nested, and large-text splits responsive", async ({ page }, testInfo) => {
  await page.goto("/?split=vertical");
  const workspaceSeparator = page.getByRole("separator", {
    name: "Resize collections and request workspace",
  });
  const responseSeparator = page.getByRole("separator", {
    name: "Resize request and response panels",
  });
  await expect(workspaceSeparator).toBeVisible();
  await expect(responseSeparator).toBeVisible();

  const idleWorkspace = await measuredDragBy(page, workspaceSeparator, 160, 0);
  expect(idleWorkspace.elapsedMs).toBeLessThan(maximumScriptedDragMs);
  expect(idleWorkspace.maximumFrameGapMs).toBeLessThan(maximumFrameGapMs);

  const responseBody = page.locator("pre");
  await responseBody.evaluate((element) => {
    element.textContent = "abcdefghij ".repeat(95_326).slice(0, 1_048_576);
    element.setAttribute("data-resize-cost", "large-text");
  });
  await expect(responseBody).toHaveText(/abcdefghij/);
  const textLayout = await responseBody.evaluate((element) => {
    const style = getComputedStyle(element);
    return {
      length: element.textContent?.length,
      whiteSpace: style.whiteSpace,
      wordBreak: style.wordBreak,
    };
  });
  expect(textLayout).toEqual({
    length: 1_048_576,
    whiteSpace: "pre",
    wordBreak: "normal",
  });

  const responseBox = await requiredBox(responseSeparator);
  await page.mouse.move(
    responseBox.x + responseBox.width / 2,
    responseBox.y + responseBox.height / 2,
  );
  await page.mouse.down();
  await page.mouse.move(responseBox.x + 40, responseBox.y + responseBox.height / 2);
  await expect(responseBody).toHaveCSS("content-visibility", "hidden");
  await page.mouse.up();
  await expect(responseBody).toHaveCSS("content-visibility", "visible");

  const response = await measuredDragBy(page, responseSeparator, 240, 0);
  expect(response.elapsedMs).toBeLessThan(maximumScriptedDragMs);
  expect(response.maximumFrameGapMs).toBeLessThan(maximumFrameGapMs);

  const workspaceBox = await requiredBox(workspaceSeparator);
  await page.mouse.move(
    workspaceBox.x + workspaceBox.width / 2,
    workspaceBox.y + workspaceBox.height / 2,
  );
  await page.mouse.down();
  await page.mouse.move(workspaceBox.x + 40, workspaceBox.y + workspaceBox.height / 2);
  await expect(responseBody).toHaveCSS("content-visibility", "hidden");
  await page.mouse.up();
  await expect(responseBody).toHaveCSS("content-visibility", "visible");

  const largeTextWorkspace = await measuredDragBy(page, workspaceSeparator, 160, 0);
  expect(largeTextWorkspace.elapsedMs).toBeLessThan(maximumScriptedDragMs);
  expect(largeTextWorkspace.maximumFrameGapMs).toBeLessThan(maximumFrameGapMs);
  const metrics = { idleWorkspace, largeTextWorkspace, response };
  await testInfo.attach("resize-metrics.json", {
    body: JSON.stringify(metrics, null, 2),
    contentType: "application/json",
  });
  console.info(`resize metrics ${JSON.stringify(metrics)}`);
});

test("honors pixel constraints without a first-drag jump", async ({ page }) => {
  await page.setViewportSize({ height: 1_080, width: 768 });
  await page.goto("/?split=vertical");
  const separator = page.getByRole("separator", {
    name: "Resize collections and request workspace",
  });
  const controlledId = await separator.getAttribute("aria-controls");
  expect(controlledId).not.toBeNull();
  const firstPanel = page.locator(`[id="${controlledId}"]`);
  const before = await requiredBox(firstPanel);
  expect(before.width).toBeGreaterThanOrEqual(219);
  await expect(separator).toHaveAttribute("aria-valuenow", "29");
  await expect(separator).toHaveAttribute("aria-valuemin", "29");
  await expect(separator).toHaveAttribute("aria-valuemax", "36");

  const separatorBox = await requiredBox(separator);
  const x = separatorBox.x + separatorBox.width / 2;
  const y = separatorBox.y + separatorBox.height / 2;
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + 1, y);
  await page.mouse.up();

  const after = await requiredBox(firstPanel);
  expect(Math.abs(after.width - before.width)).toBeLessThanOrEqual(3);
});

test("persists only a completed resize and preserves keyboard access", async ({ page }) => {
  await page.goto("/?split=vertical");
  const separator = page.getByRole("separator", {
    name: "Resize request and response panels",
  });
  const box = await requiredBox(separator);
  const storageKey = "postmite.requestResponseLayout.vertical";

  await page.evaluate((key) => localStorage.removeItem(key), storageKey);
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + 80, box.y + box.height / 2);
  await page.waitForTimeout(50);
  expect(await page.evaluate((key) => localStorage.getItem(key), storageKey)).toBeNull();
  await page.mouse.up();
  await expect.poll(() => page.evaluate((key) => localStorage.getItem(key), storageKey)).not.toBeNull();

  await separator.focus();
  await expect(separator).toBeFocused();
  await expect(separator).toHaveAttribute("aria-orientation", "vertical");
  await separator.press("Home");
  const before = Number(await separator.getAttribute("aria-valuenow"));
  await separator.press("ArrowRight");
  const after = Number(await separator.getAttribute("aria-valuenow"));
  expect(after).toBeGreaterThan(before);
});

test("keeps the raw request editor out of width-dependent wrapping", async ({ page }) => {
  await page.goto("/?split=vertical");
  await page
    .getByRole("tablist", { name: "Request option tabs" })
    .getByRole("tab", { name: "Body" })
    .click();
  const editor = page.getByRole("textbox", { name: "Raw body editor" });
  await expect(editor).toBeVisible();
  const editorHost = page.getByTestId("raw-body-editor");
  await expect(editorHost.locator(".cm-lineWrapping")).toHaveCount(0);
  await expect(editorHost.locator(".cm-scroller")).toHaveCSS("overflow-x", "auto");
});

async function dragBy(
  page: Page,
  separator: Locator,
  deltaX: number,
  deltaY: number,
) {
  const box = await requiredBox(separator);
  const x = box.x + box.width / 2;
  const y = box.y + box.height / 2;
  await page.mouse.move(x, y);
  await page.mouse.down();
  const startedAt = Date.now();
  await page.mouse.move(x + deltaX, y + deltaY, { steps: 60 });
  await page.mouse.up();
  return Date.now() - startedAt;
}

async function measuredDragBy(
  page: Page,
  separator: Locator,
  deltaX: number,
  deltaY: number,
) {
  await page.evaluate(() => {
    const metrics = { active: true, lastFrame: performance.now(), maximumFrameGapMs: 0 };
    Object.assign(window, { __postmiteResizeFrameMetrics: metrics });
    const sample = (timestamp: number) => {
      if (!metrics.active) return;
      metrics.maximumFrameGapMs = Math.max(
        metrics.maximumFrameGapMs,
        timestamp - metrics.lastFrame,
      );
      metrics.lastFrame = timestamp;
      requestAnimationFrame(sample);
    };
    requestAnimationFrame(sample);
  });
  const elapsedMs = await dragBy(page, separator, deltaX, deltaY);
  const maximumFrameGapMs = await page.evaluate(() => {
    const metrics = (window as typeof window & {
      __postmiteResizeFrameMetrics: {
        active: boolean;
        maximumFrameGapMs: number;
      };
    }).__postmiteResizeFrameMetrics;
    metrics.active = false;
    return metrics.maximumFrameGapMs;
  });
  return { elapsedMs, maximumFrameGapMs };
}

async function requiredBox(locator: Locator) {
  const box = await locator.boundingBox();
  if (!box) throw new Error("Resizable separator does not have a bounding box");
  return box;
}
