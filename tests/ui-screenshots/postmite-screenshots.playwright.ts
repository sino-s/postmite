import { expect, test } from "@playwright/test";

const outputDir = "artifacts/screenshots/ui";

test("captures request workspace screenshots", async ({ page }, testInfo) => {
  const variant = testInfo.project.name;
  const theme = variant.includes("dark") ? "dark" : "light";
  const density = variant.includes("compact") ? "compact" : "comfortable";

  await page.goto(`/?theme=${theme}&density=${density}`);
  await expect(page.getByRole("heading", { name: "Postmite" })).toBeVisible();
  await expect(page.getByLabel("Request editor screenshot fixture")).toBeVisible();
  await expect(page.getByRole("tablist", { name: "Request option tabs" })).toBeVisible();
  await expect
    .poll(() =>
      page.evaluate(() => {
        const appRoot = document.querySelector("#root");
        const main = document.querySelector("main");
        return Boolean(
          appRoot &&
            main &&
            document.body.scrollHeight <= document.body.clientHeight + 1 &&
            appRoot.scrollHeight <= appRoot.clientHeight + 1 &&
            main.scrollHeight <= main.clientHeight + 1 &&
            getComputedStyle(document.documentElement).overflowY === "hidden" &&
            getComputedStyle(document.body).overflowY === "hidden",
        );
      }),
    )
    .toBe(true);
  await expect(page.getByRole("tab", { name: "Params" })).toHaveAttribute("aria-selected", "true");
  await expect(page.getByRole("region", { name: "Response" })).toContainText(
    "Status 200",
  );
  await expect(page.getByRole("button", { name: "Stack request options above response" })).toHaveAttribute("aria-pressed", "true");
  await expect(
    page.getByRole("tablist", { name: "Response details" }).getByRole("tab", { name: "Body" }),
  ).toHaveAttribute("aria-selected", "true");
  await expect
    .poll(() =>
      page.getByRole("tablist", { name: "Response details" }).evaluate((element) => {
        return element.getBoundingClientRect().height;
      }),
    )
    .toBeLessThanOrEqual(48);
  await page.getByLabel("Search response").fill("available");
  await expect(page.locator("mark")).toHaveCount(2);

  await page.screenshot({
    animations: "disabled",
    path: `${outputDir}/${variant}.png`,
  });

  if (variant === "desktop-light") {
    await page.getByRole("button", { name: "Application menu" }).click();
    await expect(page.getByLabel("Theme")).toBeVisible();
    await expect(page.getByLabel("Density")).toBeVisible();
    await expect(page.getByRole("button", { name: "Check for updates" })).toBeVisible();
    await expect(page.getByLabel("Language")).toBeVisible();
    await page.screenshot({
      animations: "disabled",
      path: `${outputDir}/${variant}-menu.png`,
    });

    await page.goto(`/?theme=${theme}&density=${density}&split=vertical`);
    await expect(page.getByRole("button", { name: "Place request options beside response" })).toHaveAttribute("aria-pressed", "true");
    await page.screenshot({
      animations: "disabled",
      path: `${outputDir}/${variant}-vertical-split.png`,
    });

    await page.goto(`/?theme=${theme}&density=${density}&state=empty`);
    await expect(
      page.getByLabel("Empty request workspace screenshot fixture"),
    ).toBeVisible();
    await page.screenshot({
      animations: "disabled",
      path: `${outputDir}/${variant}-empty.png`,
    });
  }
});
