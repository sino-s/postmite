import { json } from "@codemirror/lang-json";
import { EditorState } from "@codemirror/state";
import {
  EditorView,
  keymap,
  lineNumbers,
  placeholder,
} from "@codemirror/view";
import { useEffect, useRef } from "react";
import { useI18n } from "../../app/i18n";

type CodeMirrorBodyEditorProps = {
  mode: "json" | "text";
  value: string;
  onChange: (value: string) => void;
};

export function CodeMirrorBodyEditor({
  mode,
  value,
  onChange,
}: CodeMirrorBodyEditorProps) {
  const { t } = useI18n();
  const hostRef = useRef<HTMLDivElement | null>(null);
  const initialValueRef = useRef(value);
  const viewRef = useRef<EditorView | null>(null);
  const onChangeRef = useRef(onChange);

  onChangeRef.current = onChange;

  useEffect(() => {
    if (!hostRef.current) {
      return undefined;
    }

    const view = new EditorView({
      parent: hostRef.current,
      state: EditorState.create({
        doc: initialValueRef.current,
        extensions: [
          lineNumbers(),
          keymap.of([]),
          placeholder(t("body.rawPlaceholder")),
          EditorView.lineWrapping,
          mode === "json" ? json() : [],
          EditorView.updateListener.of((update) => {
            if (update.docChanged) {
              onChangeRef.current(update.state.doc.toString());
            }
          }),
          EditorView.theme({
            "&": {
              minHeight: "240px",
              fontSize: "13px",
            },
            ".cm-content": {
              fontFamily:
                'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace',
              minHeight: "240px",
              padding: "12px",
            },
            ".cm-scroller": {
              overflow: "auto",
            },
          }),
        ],
      }),
    });

    viewRef.current = view;

    return () => {
      view.destroy();
      viewRef.current = null;
    };
  }, [mode, t]);

  useEffect(() => {
    const view = viewRef.current;
    if (!view || view.state.doc.toString() === value) {
      return;
    }

    view.dispatch({
      changes: {
        from: 0,
        to: view.state.doc.length,
        insert: value,
      },
    });
  }, [value]);

  return (
    <div
      ref={hostRef}
      aria-label={t("body.rawEditor")}
      className="min-h-60 overflow-hidden rounded-md border border-slate-300 bg-white focus-within:border-sky-500 focus-within:outline focus-within:outline-2 focus-within:outline-sky-500"
    />
  );
}
