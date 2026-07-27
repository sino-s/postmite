import { lazy, Suspense, useState } from "react";

const CodeMirrorBodyEditor = lazy(async () => {
  const module = await import("./CodeMirrorBodyEditor");
  return { default: module.CodeMirrorBodyEditor };
});

type RawBodyEditorProps = {
  value: string;
  onChange: (value: string) => void;
};

export function RawBodyEditor({ value, onChange }: RawBodyEditorProps) {
  const [mode, setMode] = useState<"json" | "text">("json");

  return (
    <section className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="flex items-center justify-between gap-3">
        <h2 className="text-sm font-semibold text-slate-950">Raw Body</h2>
        <div
          aria-label="Body editor mode"
          className="inline-flex rounded-md border border-slate-300 bg-white p-0.5"
          role="group"
        >
          {(["json", "text"] as const).map((item) => (
            <button
              aria-pressed={mode === item}
              className="rounded px-3 py-1 text-xs font-medium text-slate-700 hover:bg-slate-100 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500 aria-pressed:bg-slate-900 aria-pressed:text-white"
              key={item}
              onClick={() => setMode(item)}
              type="button"
            >
              {item.toUpperCase()}
            </button>
          ))}
        </div>
      </div>
      <Suspense
        fallback={
          <textarea
            aria-label="Raw body"
            className="min-h-60 resize-none rounded-md border border-slate-300 bg-white p-3 font-mono text-sm focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
            onChange={(event) => onChange(event.currentTarget.value)}
            value={value}
          />
        }
      >
        <CodeMirrorBodyEditor mode={mode} onChange={onChange} value={value} />
      </Suspense>
    </section>
  );
}
