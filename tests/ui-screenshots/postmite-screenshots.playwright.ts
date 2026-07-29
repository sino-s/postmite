import { expect, test } from "@playwright/test";

const outputDir = "artifacts/screenshots/ui";

test("captures request workspace screenshots", async ({ page }, testInfo) => {
  const variant = testInfo.project.name;
  const theme = variant.includes("dark") ? "dark" : "light";
  const density = variant.includes("compact") ? "compact" : "comfortable";

  await page.goto(`/?theme=${theme}&density=${density}`);
  await expect(page.getByRole("heading", { name: "Postmite" })).toBeVisible();
  await expect(page.getByLabel("Request editor screenshot fixture")).toBeVisible();
  await expect(page.getByRole("region", { name: "Response" })).toContainText(
    "Status 200",
  );
  await expect(page.getByRole("region", { name: "Cookie jar" })).toContainText(
    "No cookies",
  );

  await page.screenshot({
    animations: "disabled",
    fullPage: true,
    path: `${outputDir}/${variant}.png`,
  });

  if (variant === "desktop-light") {
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
