import { Braces } from "lucide-react";
import { lazy, Suspense, useRef, useState } from "react";
import { useI18n } from "../../../app/i18n";
import { IconButton } from "../controls/IconButton";
import type { CodeMirrorBodyEditorHandle } from "./CodeMirrorBodyEditor";
import type { JsonValidation } from "./json-document";

const CodeMirrorBodyEditor = lazy(async () => {
  const module = await import("./CodeMirrorBodyEditor");
  return { default: module.CodeMirrorBodyEditor };
});

type RawBodyEditorProps = {
  value: string;
  onChange: (value: string) => void;
};

export function RawBodyEditor({ value, onChange }: RawBodyEditorProps) {
  const { t } = useI18n();
  const [mode, setMode] = useState<"json" | "text">("json");
  const [validation, setValidation] = useState<JsonValidation>(() => ({ state: "empty" }));
  const editorRef = useRef<CodeMirrorBodyEditorHandle | null>(null);

  function selectMode(nextMode: "json" | "text") {
    setMode(nextMode);
    if (nextMode === "text") {
      setValidation({ state: "empty" });
    }
  }

  return (
    <section className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h2 className="text-sm font-semibold text-slate-950">{t("body.raw")}</h2>
        <div className="flex min-w-0 flex-wrap items-center justify-end gap-2">
          {mode === "json" ? (
            <IconButton
              className="size-8 shrink-0 focus-visible:ring-2 focus-visible:ring-sky-500"
              disabled={validation.state !== "valid"}
              label={t("body.formatJson")}
              onClick={() => editorRef.current?.format()}
            >
              <Braces aria-hidden="true" size={16} />
            </IconButton>
          ) : null}
          <div
            aria-label={t("body.editorMode")}
            className="inline-flex shrink-0 rounded-md border border-slate-300 bg-white p-0.5"
            role="group"
          >
            {(["json", "text"] as const).map((item) => (
              <button
                aria-pressed={mode === item}
                className="rounded px-3 py-1 text-xs font-medium text-slate-700 hover:bg-slate-100 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500 aria-pressed:bg-slate-900 aria-pressed:text-white"
                key={item}
                onClick={() => selectMode(item)}
                type="button"
              >
                {item.toUpperCase()}
              </button>
            ))}
          </div>
        </div>
      </div>
      <p
        aria-live="polite"
        className="min-h-5 text-xs text-red-700"
        data-testid="json-validation-summary"
      >
        {mode === "json" && validation.state === "invalid"
          ? t("body.jsonInvalid", {
              line: validation.line,
              column: validation.column,
            })
          : ""}
      </p>
      <Suspense
        fallback={
          <textarea
            aria-label={t("body.raw")}
            className="min-h-60 resize-none rounded-md border border-slate-300 bg-white p-3 font-mono text-sm focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
            onChange={(event) => onChange(event.currentTarget.value)}
            value={value}
          />
        }
      >
        <CodeMirrorBodyEditor
          mode={mode}
          onChange={onChange}
          onValidationChange={setValidation}
          ref={editorRef}
          value={value}
        />
      </Suspense>
    </section>
  );
}
