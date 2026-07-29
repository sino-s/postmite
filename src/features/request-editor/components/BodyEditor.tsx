import { Plus, Trash2 } from "lucide-react";

import { describeBodyFile } from "../../../shared/api/requests";
import type { BodyFileReferenceDto, MultipartPartDto, RequestBodyDto } from "../../../shared/api/generated/ipc";
import { RawBodyEditor } from "../RawBodyEditor";
import {
  bodyModeLabel,
  emptyBodyForMode,
  emptyMultipartFilePart,
} from "../request-editor-model";
import { FieldTable } from "./FieldTable";
import { IconButton } from "./IconButton";
import { useI18n } from "../../../app/i18n";

type BodyEditorProps = {
  body: RequestBodyDto;
  onChange: (body: RequestBodyDto) => void;
  workspaceId: string;
};

export function BodyEditor({ body, onChange, workspaceId }: BodyEditorProps) {
  const { t } = useI18n();
  return (
    <section className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="flex items-center justify-between gap-3">
        <h2 className="text-sm font-semibold text-slate-950">{t("body.title")}</h2>
        <div
          aria-label={t("body.mode")}
          className="inline-flex flex-wrap rounded-md border border-slate-300 bg-white p-0.5"
          role="group"
        >
          {(["NONE", "RAW", "URL_ENCODED", "MULTIPART", "BINARY"] as const).map(
            (mode) => (
              <button
                aria-pressed={body.type === mode}
                className="rounded px-2 py-1 text-xs font-medium text-slate-700 hover:bg-slate-100 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500 aria-pressed:bg-slate-900 aria-pressed:text-white"
                key={mode}
                onClick={() => onChange(emptyBodyForMode(mode, body))}
                type="button"
              >
                {bodyModeLabel(mode)}
              </button>
            ),
          )}
        </div>
      </div>
      {body.type === "NONE" ? (
        <div className="min-h-60 rounded-md border border-dashed border-slate-300 bg-white p-3 text-sm text-slate-500" />
      ) : null}
      {body.type === "RAW" ? (
        <RawBodyEditor
          onChange={(content) => onChange({ type: "RAW", content })}
          value={body.content}
        />
      ) : null}
      {body.type === "URL_ENCODED" ? (
        <FieldTable
          fields={body.fields}
          legend={t("body.urlEncoded")}
          onChange={(fields) => onChange({ type: "URL_ENCODED", fields })}
        />
      ) : null}
      {body.type === "MULTIPART" ? (
        <MultipartEditor
          onChange={(parts) => onChange({ type: "MULTIPART", parts })}
          parts={body.parts}
          workspaceId={workspaceId}
        />
      ) : null}
      {body.type === "BINARY" ? (
        <BodyFileEditor
          file={body.file}
          onChange={(file) => onChange({ type: "BINARY", file })}
          workspaceId={workspaceId}
        />
      ) : null}
    </section>
  );
}

function MultipartEditor({
  onChange,
  parts,
  workspaceId,
}: {
  onChange: (parts: MultipartPartDto[]) => void;
  parts: MultipartPartDto[];
  workspaceId: string;
}) {
  const { t } = useI18n();
  const fieldParts = parts.filter((part) => part.type === "FIELD");
  const fileParts = parts.filter((part) => part.type === "FILE");
  return (
    <div className="grid min-h-60 gap-3">
      <FieldTable
        fields={fieldParts.map((part) => ({
          enabled: part.enabled,
          order: part.order,
          name: part.name,
          value: part.value,
        }))}
        legend={t("body.multipartFields")}
        onChange={(fields) =>
          onChange([
            ...fields.map((field) => ({ type: "FIELD" as const, ...field })),
            ...fileParts,
          ])
        }
      />
      <div className="grid gap-2 rounded-md border border-slate-300 bg-white p-3">
        <div className="flex items-center justify-between gap-2">
          <h3 className="text-sm font-semibold">{t("body.multipartFiles")}</h3>
          <button
            className="inline-flex h-8 items-center gap-2 rounded-md border border-slate-300 px-2 text-xs font-medium hover:bg-slate-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
            onClick={() => onChange([...parts, emptyMultipartFilePart(parts.length)])}
            type="button"
          >
            <Plus aria-hidden="true" size={14} />
            {t("body.file")}
          </button>
        </div>
        {fileParts.map((part, index) => (
          <BodyFileEditor
            file={part.file}
            key={`${part.order}-${index}`}
            name={part.name}
            onChange={(file, name = part.name) =>
              onChange(
                parts.map((item) =>
                  item === part ? { ...part, name, file } : item,
                ),
              )
            }
            onDelete={() => onChange(parts.filter((item) => item !== part))}
            workspaceId={workspaceId}
          />
        ))}
      </div>
    </div>
  );
}

function BodyFileEditor({
  file,
  name,
  onChange,
  onDelete,
  workspaceId,
}: {
  file: BodyFileReferenceDto;
  name?: string;
  onChange: (file: BodyFileReferenceDto, name?: string) => void;
  onDelete?: () => void;
  workspaceId: string;
}) {
  const { t } = useI18n();
  const pathValue = file.path.path;
  const pathKind = file.path.type;
  async function handleRefresh() {
    const absolutePath =
      pathKind === "ABSOLUTE"
        ? pathValue
        : window.prompt(t("app.replacementBodyPath"), pathValue)?.trim();
    if (!absolutePath) {
      return;
    }
    const nextFile = await describeBodyFile({
      workspaceId,
      path: absolutePath,
    });
    onChange(nextFile, name);
  }

  return (
    <div className="grid gap-2 rounded-md border border-slate-300 bg-white p-3">
      <div className="grid gap-2 md:grid-cols-[160px_minmax(0,1fr)]">
        {name !== undefined ? (
          <input
            aria-label="Multipart file field name"
            className="h-9 min-w-0 rounded-md border border-slate-300 px-2 text-sm focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
            onChange={(event) => onChange(file, event.currentTarget.value)}
            placeholder="field name"
            value={name}
          />
        ) : null}
        <input
          aria-label="Body file path"
          className="h-9 min-w-0 rounded-md border border-slate-300 px-2 font-mono text-sm focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
          onChange={(event) =>
            onChange({
              ...file,
              path:
                pathKind === "RELATIVE"
                  ? { type: "RELATIVE", path: event.currentTarget.value }
                  : { type: "ABSOLUTE", path: event.currentTarget.value },
            })
          }
          placeholder={pathKind === "RELATIVE" ? "payloads/body.bin" : "/tmp/body.bin"}
          value={pathValue}
        />
      </div>
      <div className="grid gap-2 md:grid-cols-[120px_minmax(0,1fr)_120px_160px_minmax(0,1fr)_auto]">
        <select
          aria-label="Body file path kind"
          className="h-9 rounded-md border border-slate-300 bg-white px-2 text-sm focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
          onChange={(event) =>
            onChange({
              ...file,
              path:
                event.currentTarget.value === "RELATIVE"
                  ? { type: "RELATIVE", path: pathValue }
                  : { type: "ABSOLUTE", path: pathValue },
            })
          }
          value={pathKind}
        >
          <option value="RELATIVE">Relative</option>
          <option value="ABSOLUTE">Absolute</option>
        </select>
        <input
          aria-label="Body file name"
          className="h-9 min-w-0 rounded-md border border-slate-300 px-2 text-sm focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
          onChange={(event) => onChange({ ...file, fileName: event.currentTarget.value })}
          placeholder="body.bin"
          value={file.fileName}
        />
        <input
          aria-label="Body file size"
          className="h-9 min-w-0 rounded-md border border-slate-300 px-2 text-sm focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
          min="0"
          onChange={(event) =>
            onChange({ ...file, size: BigInt(event.currentTarget.value || "0") })
          }
          type="number"
          value={file.size.toString()}
        />
        <input
          aria-label="Body file modified time"
          className="h-9 min-w-0 rounded-md border border-slate-300 px-2 text-sm focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
          onChange={(event) =>
            onChange({
              ...file,
              modifiedAtEpochSeconds: event.currentTarget.value
                ? BigInt(event.currentTarget.value)
                : null,
            })
          }
          placeholder="mtime"
          value={file.modifiedAtEpochSeconds?.toString() ?? ""}
        />
        <input
          aria-label="Body file hash"
          className="h-9 min-w-0 rounded-md border border-slate-300 px-2 font-mono text-sm focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
          onChange={(event) => onChange({ ...file, sha256: event.currentTarget.value })}
          placeholder="sha256"
          value={file.sha256}
        />
        {onDelete ? (
          <IconButton label="Delete file part" onClick={onDelete}>
            <Trash2 aria-hidden="true" size={14} />
          </IconButton>
        ) : null}
        <button
          className="inline-flex h-9 items-center justify-center rounded-md border border-slate-300 px-2 text-xs font-medium hover:bg-slate-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
          onClick={() => void handleRefresh()}
          type="button"
        >
          {t("body.refresh")}
        </button>
      </div>
    </div>
  );
}
