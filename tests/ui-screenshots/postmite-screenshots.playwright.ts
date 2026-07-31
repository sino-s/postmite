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
  const curlCopy = page.getByRole("button", { name: "Copy cURL" });
  await expect(curlCopy).toBeVisible();
  await expect
    .poll(async () => {
      const actionBox = await curlCopy.boundingBox();
      const tabBox = await page
        .getByRole("tablist", { name: "Request option tabs" })
        .boundingBox();
      return Boolean(
        actionBox &&
          tabBox &&
          actionBox.x >= tabBox.x + tabBox.width &&
          actionBox.x + actionBox.width <= page.viewportSize()!.width,
      );
    })
    .toBe(true);
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

  await page
    .getByRole("tablist", { name: "Request option tabs" })
    .getByRole("tab", { name: "Body" })
    .click();
  const rawEditor = page.getByRole("textbox", { name: "Raw body editor" });
  const formatJson = page.getByRole("button", { name: "Format JSON" });
  await expect(rawEditor).toBeVisible();
  await expect(formatJson).toBeVisible();

  const precisionSource =
    '{"unsafe":9007199254740993,"decimal":1.2300,"exponent":6.02e+23,"same":1,"same":2,"escaped":"\\\\u0041"}';
  await rawEditor.fill(precisionSource);
  await expect(formatJson).toBeEnabled();
  await formatJson.hover();
  await expect(page.getByRole("tooltip")).toContainText("Format JSON");
  await formatJson.focus();
  await formatJson.press("Enter");
  await expect(rawEditor).toContainText("9007199254740993");
  await expect(rawEditor).toContainText("1.2300");
  await expect(rawEditor).toContainText("6.02e+23");
  await expect(rawEditor).toContainText("\\u0041");
  await expect(rawEditor.locator(".cm-line").filter({ hasText: '"same"' })).toHaveCount(2);
  await formatJson.focus();
  await page.screenshot({
    animations: "disabled",
    path: `${outputDir}/${variant}-json-valid-keyboard.png`,
  });

  await rawEditor.press("Control+z");
  await expect(rawEditor).toHaveText(precisionSource);

  await rawEditor.fill('{\n  "valid": true,\n  "broken":\n}');
  await expect(page.getByTestId("json-validation-summary")).toContainText(
    "Invalid JSON at line",
  );
  await expect(formatJson).toBeDisabled();
  await page.getByRole("heading", { name: "Raw Body" }).scrollIntoViewIfNeeded();
  await page.screenshot({
    animations: "disabled",
    path: `${outputDir}/${variant}-json-invalid.png`,
  });

  await rawEditor.fill("   ");
  await expect(formatJson).toBeDisabled();
  await expect(page.getByTestId("json-validation-summary")).toBeEmpty();
  await page.getByRole("heading", { name: "Raw Body" }).scrollIntoViewIfNeeded();
  await page.screenshot({
    animations: "disabled",
    path: `${outputDir}/${variant}-json-empty.png`,
  });

  await page.getByRole("button", { name: "TEXT" }).click();
  await expect(formatJson).not.toBeVisible();
  await expect(page.getByTestId("json-validation-summary")).toBeEmpty();
  await page.getByRole("heading", { name: "Raw Body" }).scrollIntoViewIfNeeded();
  await page.screenshot({
    animations: "disabled",
    path: `${outputDir}/${variant}-text-mode.png`,
  });

  if (variant === "desktop-light") {
    await curlCopy.focus();
    await expect(curlCopy).toBeFocused();
    await page.screenshot({
      animations: "disabled",
      path: `${outputDir}/${variant}-curl-focus.png`,
    });

    await page.goto(`/?theme=${theme}&density=${density}&curl=confirm`);
    await expect(
      page.getByRole("alertdialog", { name: "This cURL contains Secret values" }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Copy redacted cURL" }),
    ).toBeFocused();
    await page.screenshot({
      animations: "disabled",
      path: `${outputDir}/${variant}-curl-confirmation.png`,
    });

    await page.goto(`/?theme=${theme}&density=${density}`);
    await page.getByRole("button", { name: "Application menu" }).click();
    await expect(page.getByLabel("Theme")).toBeVisible();
    await expect(page.getByLabel("Density")).toBeVisible();
    await expect(page.getByRole("button", { name: "Check for updates" })).toBeVisible();
    await expect(page.getByLabel("Language")).toBeVisible();
    await page.screenshot({
      animations: "disabled",
      path: `${outputDir}/${variant}-menu.png`,
    });

    await page.goto(`/?theme=${theme}&density=${density}&manager=workspace`);
    const workspaceDialog = page.getByRole("dialog", {
      name: "Workspace management",
    });
    await expect(workspaceDialog).toBeVisible();
    await workspaceDialog.locator("#managed-workspace").focus();
    await workspaceDialog.locator("#managed-workspace").press("Tab");
    await expect(
      workspaceDialog.getByLabel("Rename selected workspace"),
    ).toBeFocused();
    await page.screenshot({
      animations: "disabled",
      path: `${outputDir}/${variant}-workspace-management.png`,
    });

    await page.goto(`/?theme=${theme}&density=${density}&manager=environment`);
    const environmentDialog = page.getByRole("dialog", {
      name: "Environment management",
    });
    await expect(environmentDialog).toBeVisible();
    await expect(environmentDialog.getByLabel("Variable value 2")).toHaveAttribute(
      "type",
      "password",
    );
    await expect(environmentDialog.getByLabel("Variable value 2")).toHaveValue("");
    await environmentDialog.getByLabel("Variable name 1").focus();
    await environmentDialog.getByLabel("Variable name 1").press("Tab");
    await expect(environmentDialog.getByLabel("Variable type 1")).toBeFocused();
    await page.screenshot({
      animations: "disabled",
      path: `${outputDir}/${variant}-environment-management.png`,
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
