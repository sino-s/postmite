import { json } from "@codemirror/lang-json";
import { history, historyKeymap, isolateHistory } from "@codemirror/commands";
import { linter, lintGutter, type Diagnostic } from "@codemirror/lint";
import {
  EditorSelection,
  EditorState,
  Transaction,
} from "@codemirror/state";
import {
  EditorView,
  keymap,
  lineNumbers,
  placeholder,
} from "@codemirror/view";
import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useRef,
} from "react";
import { useI18n } from "../../../app/i18n";
import {
  formatJsonDocument,
  validateJsonDocument,
  type JsonValidation,
} from "./json-document";

type CodeMirrorBodyEditorProps = {
  mode: "json" | "text";
  value: string;
  onChange: (value: string) => void;
  onValidationChange: (validation: JsonValidation) => void;
};

export type CodeMirrorBodyEditorHandle = {
  format: () => void;
  focus: () => void;
};

export const CodeMirrorBodyEditor = forwardRef<
  CodeMirrorBodyEditorHandle,
  CodeMirrorBodyEditorProps
>(function CodeMirrorBodyEditor(
  { mode, value, onChange, onValidationChange },
  ref,
) {
  const { t } = useI18n();
  const hostRef = useRef<HTMLDivElement | null>(null);
  const viewRef = useRef<EditorView | null>(null);
  const onChangeRef = useRef(onChange);
  const onValidationChangeRef = useRef(onValidationChange);
  const valueRef = useRef(value);

  onChangeRef.current = onChange;
  onValidationChangeRef.current = onValidationChange;
  valueRef.current = value;

  useImperativeHandle(ref, () => ({
    focus() {
      viewRef.current?.focus();
    },
    format() {
      const view = viewRef.current;
      if (!view || mode !== "json") {
        return;
      }

      const current = view.state.doc.toString();
      const validation = validateJsonDocument(current);
      onValidationChangeRef.current(validation);
      if (validation.state !== "valid") {
        return;
      }

      const formatted = formatJsonDocument(current);
      if (formatted === null || formatted === current) {
        return;
      }

      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: formatted },
        selection: EditorSelection.cursor(Math.min(view.state.selection.main.head, formatted.length)),
        scrollIntoView: true,
        annotations: isolateHistory.of("full"),
      });
      view.focus();
    },
  }), [mode]);

  useEffect(() => {
    if (!hostRef.current) {
      return undefined;
    }

    const view = new EditorView({
      parent: hostRef.current,
      state: EditorState.create({
        doc: valueRef.current,
        extensions: [
          lineNumbers(),
          history(),
          keymap.of(historyKeymap),
          placeholder(t("body.rawPlaceholder")),
          EditorView.lineWrapping,
          EditorView.contentAttributes.of({
            "aria-label": t("body.rawEditor"),
          }),
          mode === "json"
            ? [
                json(),
                lintGutter(),
                linter(
                  (view): Diagnostic[] => {
                    const validation = validateJsonDocument(view.state.doc.toString());
                    onValidationChangeRef.current(validation);
                    if (validation.state !== "invalid") {
                      return [];
                    }
                    return [
                      {
                        from: validation.diagnosticFrom,
                        to: validation.diagnosticTo,
                        severity: "error",
                        message: t("body.jsonInvalid", {
                          line: validation.line,
                          column: validation.column,
                        }),
                      },
                    ];
                  },
                  { delay: 150 },
                ),
              ]
            : [],
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
      annotations: Transaction.addToHistory.of(false),
    });
  }, [value]);

  return (
    <div
      ref={hostRef}
      data-testid="raw-body-editor"
      className="min-h-60 overflow-hidden rounded-md border border-slate-300 bg-white focus-within:border-sky-500 focus-within:outline focus-within:outline-2 focus-within:outline-sky-500"
    />
  );
});
