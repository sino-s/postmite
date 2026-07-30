import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "jest-axe";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import { I18nProvider } from "../../../app/i18n";
import { RawBodyEditor } from "./RawBodyEditor";

function ControlledEditor({
  initialValue,
  onChange = () => undefined,
}: {
  initialValue: string;
  onChange?: (value: string) => void;
}) {
  const [value, setValue] = useState(initialValue);
  return (
    <I18nProvider>
      <RawBodyEditor
        onChange={(nextValue) => {
          setValue(nextValue);
          onChange(nextValue);
        }}
        value={value}
      />
    </I18nProvider>
  );
}

function codeMirrorContent(container: HTMLElement) {
  const content = container.querySelector<HTMLElement>(".cm-content");
  if (!content) {
    throw new Error("CodeMirror content not found");
  }
  return content;
}

describe("RawBodyEditor JSON tools", () => {
  it("formats through onChange and restores the unsaved text with one undo", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    const { container } = render(
      <ControlledEditor initialValue='{"unsafe":9007199254740993,"nested":[true]}' onChange={onChange} />,
    );
    const format = await screen.findByRole("button", { name: "Format JSON" });
    await waitFor(() => expect(format).toBeEnabled());

    await user.click(format);
    await waitFor(() =>
      expect(onChange).toHaveBeenLastCalledWith(`{
  "unsafe": 9007199254740993,
  "nested": [
    true
  ]
}`),
    );

    await user.keyboard("{Control>}z{/Control}");
    await waitFor(() =>
      expect(onChange).toHaveBeenLastCalledWith(
        '{"unsafe":9007199254740993,"nested":[true]}',
      ),
    );
    expect(codeMirrorContent(container)).toHaveTextContent(
      '{"unsafe":9007199254740993,"nested":[true]}',
    );
  });

  it("shows location-aware diagnostics and clears them in TEXT mode", async () => {
    const user = userEvent.setup();
    render(<ControlledEditor initialValue={'{\n  "ok": true,\n  "broken":\n}'} />);

    expect(
      await screen.findByText("Invalid JSON at line 4, column 1."),
    ).toBeVisible();
    expect(screen.getByRole("button", { name: "Format JSON" })).toBeDisabled();

    await user.click(screen.getByRole("button", { name: "TEXT" }));
    expect(screen.queryByRole("button", { name: "Format JSON" })).not.toBeInTheDocument();
    expect(screen.queryByText("Invalid JSON at line 4, column 1.")).not.toBeInTheDocument();
  });

  it("keeps empty JSON neutral and unformattable", async () => {
    render(<ControlledEditor initialValue=" \n\t" />);
    const format = await screen.findByRole("button", { name: "Format JSON" });
    expect(format).toBeDisabled();
    expect(screen.getByTestId("json-validation-summary")).toBeEmptyDOMElement();
  });

  it("clears diagnostics when an edit makes the JSON valid", async () => {
    const user = userEvent.setup();
    const { container } = render(<ControlledEditor initialValue='{"broken":}' />);
    expect(await screen.findByText(/Invalid JSON at line/)).toBeVisible();

    const editor = codeMirrorContent(container);
    await user.click(editor);
    await user.keyboard("{Control>}a{/Control}");
    await user.paste('{"ok":true}');

    await waitFor(() =>
      expect(screen.getByTestId("json-validation-summary")).toBeEmptyDOMElement(),
    );
    expect(screen.getByRole("button", { name: "Format JSON" })).toBeEnabled();
  });

  it("revalidates the exact current document when the displayed result is stale", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    const { container } = render(
      <ControlledEditor initialValue='{"valid":true}' onChange={onChange} />,
    );
    const format = await screen.findByRole("button", { name: "Format JSON" });
    await waitFor(() => expect(format).toBeEnabled());

    const editor = codeMirrorContent(container);
    await user.click(editor);
    await user.keyboard("{Control>}a{/Control}{{}");
    expect(format).toBeEnabled();
    onChange.mockClear();
    await user.click(format);

    expect(onChange).not.toHaveBeenCalled();
    expect(editor).toHaveTextContent("{");
  });

  it("activates formatting from the keyboard", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<ControlledEditor initialValue='{"keyboard":[1,2]}' onChange={onChange} />);
    const format = await screen.findByRole("button", { name: "Format JSON" });
    await waitFor(() => expect(format).toBeEnabled());

    format.focus();
    await user.keyboard("{Enter}");

    await waitFor(() => expect(onChange).toHaveBeenCalledWith(`{
  "keyboard": [
    1,
    2
  ]
}`));
  });

  it("has no automated accessibility violations in valid JSON mode", async () => {
    const { container } = render(<ControlledEditor initialValue='{"ok":true}' />);
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Format JSON" })).toBeEnabled(),
    );

    expect((await axe(container)).violations).toEqual([]);
  });
});
