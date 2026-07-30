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
  await expect(page.getByRole("tab", { name: "Params" })).toHaveAttribute("aria-selected", "true");
  await expect(page.getByRole("region", { name: "Response" })).toContainText(
    "Status 200",
  );
  await expect(page.getByRole("button", { name: "Stack request options above response" })).toHaveAttribute("aria-pressed", "true");

  await page.screenshot({
    animations: "disabled",
    fullPage: true,
    path: `${outputDir}/${variant}.png`,
  });

  if (variant === "desktop-light") {
    await page.goto(`/?theme=${theme}&density=${density}&split=vertical`);
    await expect(page.getByRole("button", { name: "Place request options beside response" })).toHaveAttribute("aria-pressed", "true");
    await page.screenshot({
      animations: "disabled",
      fullPage: true,
      path: `${outputDir}/${variant}-vertical-split.png`,
    });

    await page.goto(`/?theme=${theme}&density=${density}&state=empty`);
    await expect(
      page.getByLabel("Empty request workspace screenshot fixture"),
    ).toBeVisible();
    await page.screenshot({
      animations: "disabled",
      fullPage: true,
      path: `${outputDir}/${variant}-empty.png`,
    });
  }
});
