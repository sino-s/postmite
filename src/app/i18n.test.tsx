import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import {
  detectLocale,
  I18nProvider,
  translationKeyParity,
  useI18n,
} from "./i18n";

function LocaleFixture() {
  const { formatError, locale, setLocale, t } = useI18n();
  return (
    <>
      <input aria-label="draft" defaultValue="unsaved request" />
      <select aria-label="language" onChange={(event) => setLocale(event.currentTarget.value as "en" | "ja")} value={locale}>
        <option value="en">English</option>
        <option value="ja">Japanese</option>
      </select>
      <p>{t("request.send")}</p>
      <p>{t("body.formatJson")}</p>
      <p>{t("body.jsonInvalid", { line: 3, column: 7 })}</p>
      <p>{formatError({ code: "STATE_UNAVAILABLE" })}</p>
    </>
  );
}

function ErrorFixture() {
  const { formatBytes, formatError } = useI18n();
  const codes = ["INVALID_INPUT", "WORKSPACE_NOT_FOUND", "WORKSPACE_ALREADY_EXISTS", "CANNOT_DELETE_LAST_WORKSPACE", "REQUEST_NOT_FOUND", "SAVED_REQUEST_ALREADY_OPEN", "PERSISTENCE_UNAVAILABLE", "STATE_UNAVAILABLE"];
  return <><p>{formatBytes(9007199254740993n)}</p>{codes.map((code) => <p data-testid={code} key={code}>{formatError({ code })}</p>)}</>;
}

describe("i18n", () => {
  it("keeps Japanese and English catalogs in parity", () => {
    expect(translationKeyParity()).toBe(true);
  });

  it("uses English for unsupported operating-system locales", () => {
    expect(detectLocale("fr-FR")).toBe("en");
    expect(detectLocale("ja-JP")).toBe("ja");
  });

  it("maps every stable IPC error code and preserves unsafe bigint byte values", () => {
    render(<I18nProvider><ErrorFixture /></I18nProvider>);
    expect(screen.getByText("9007199254740993 B")).toBeInTheDocument();
    ["INVALID_INPUT", "WORKSPACE_NOT_FOUND", "WORKSPACE_ALREADY_EXISTS", "CANNOT_DELETE_LAST_WORKSPACE", "REQUEST_NOT_FOUND", "SAVED_REQUEST_ALREADY_OPEN", "PERSISTENCE_UNAVAILABLE", "STATE_UNAVAILABLE"].forEach((code) => expect(screen.getByTestId(code)).not.toHaveTextContent("The operation could not be completed. Try again."));
  });

  it("switches language without remounting request draft controls", async () => {
    const user = userEvent.setup();
    render(<I18nProvider><LocaleFixture /></I18nProvider>);

    await user.selectOptions(screen.getByLabelText("language"), "ja");

    expect(screen.getByText("送信")).toBeInTheDocument();
    expect(screen.getByText("JSON を整形")).toBeInTheDocument();
    expect(screen.getByText("JSON が正しくありません（3 行、7 列）。")).toBeInTheDocument();
    expect(screen.getByText("アプリケーションの状態を利用できません。再試行してください。")).toBeInTheDocument();
    expect(screen.getByLabelText("draft")).toHaveValue("unsaved request");
  });
});
