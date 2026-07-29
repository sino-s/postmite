import { useEffect, useState } from "react";

import { Button } from "../../../components/ui/button";
import { Input } from "../../../components/ui/input";
import { saveResponseFile } from "../../../shared/api/execution";
import type { ResponseExecutionState } from "../../../shared/api/execution";
import {
  createResponseViewerModel,
  formatByteCount,
  htmlSandboxSource,
  svgSandboxSource,
} from "../response-viewer-model";
import {
  prepareStructuredViewerAsync,
} from "../response-viewer-worker-client";
import { formatBodyPreview, formatProxyMetadata } from "../request-editor-model";
import type { StructuredViewerResult } from "../response-viewer-worker-core";
import { useI18n } from "../../../app/i18n";

type ResponsePanelProps = {
  execution: ResponseExecutionState | null;
};

type BodyViewMode = "pretty" | "raw" | "preview";

export function ResponsePanel({ execution }: ResponsePanelProps) {
  const { formatBytes, formatNumber, t } = useI18n();
  if (!execution) {
    return (
      <section
        aria-label={t("response.title")}
        aria-live="polite"
        className="min-h-40 min-w-0 rounded-md border border-border bg-card p-3 text-sm text-muted-foreground"
      >
        {t("response.empty")}
      </section>
    );
  }

  const elapsedMs =
    (execution.completedAtMs ?? Date.now()) - execution.startedAtMs;
  const phaseLabel = execution.phase[0].toUpperCase() + execution.phase.slice(1);
  const viewer = createResponseViewerModel(execution);

  return (
    <section
      aria-label={t("response.title")}
      aria-live="polite"
      className="grid min-h-40 min-w-0 gap-3 overflow-hidden rounded-md border border-border bg-card p-3 text-sm text-card-foreground"
    >
      <div className="flex min-w-0 flex-wrap items-center gap-3">
        <span className="rounded-md border border-border px-2 py-1 font-medium text-muted-foreground">
          {phaseLabel}
        </span>
        {execution.status ? (
          <span className="font-semibold text-foreground">
            {t("response.status", { status: execution.status })}
          </span>
        ) : null}
        <span className="text-muted-foreground">{t("response.time", { value: `${formatNumber(Math.max(0, elapsedMs))} ms` })}</span>
        {execution.protocol ? (
          <span className="text-muted-foreground">{execution.protocol}</span>
        ) : null}
        <span className="text-muted-foreground">
          {t("response.timing", { value: formatTiming(execution) })}
        </span>
        {execution.downloadProgress ? (
          <span className="text-muted-foreground">
            {t("response.received", { value: formatBytes(execution.downloadProgress.receivedBytes) })}
          </span>
        ) : null}
        {execution.decodedBytes !== null ? (
          <span className="text-muted-foreground">
            {t("response.decoded", { value: formatBytes(execution.decodedBytes) })}
          </span>
        ) : null}
        {execution.wireBytes !== null ? (
          <span className="text-muted-foreground">
            {t("response.wire", { value: formatBytes(execution.wireBytes) })}
          </span>
        ) : null}
        {execution.responseFile ? (
          <span className="text-muted-foreground">
            {formatBytes(execution.responseFile.byteCount)}
          </span>
        ) : null}
        {execution.uploadProgress ? (
          <span className="text-muted-foreground">
            {t("response.sent", { value: formatBytes(execution.uploadProgress.sentBytes) })}
          </span>
        ) : null}
        {execution.proxy ? (
          <span className="text-muted-foreground">
            Proxy {formatProxyMetadata(execution.proxy)}
          </span>
        ) : null}
        {execution.tlsVerification === false ? (
          <span className="rounded-md border border-amber-300 bg-amber-50 px-2 py-1 text-xs font-semibold text-amber-900">
            TLS verification off
          </span>
        ) : null}
      </div>

      {execution.error ? (
        <p className="rounded-md border border-red-200 bg-red-50 px-3 py-2 text-red-800">
          {execution.error}
        </p>
      ) : null}

      {execution.redirects.length > 0 ? (
        <div className="rounded-md border border-slate-200">
          <div className="border-b border-slate-200 bg-slate-50 px-2 py-2 text-xs font-semibold text-slate-600">
            Redirects
          </div>
          <div className="max-h-28 overflow-auto">
            {execution.redirects.map((redirect, index) => (
              <div
                className="grid gap-1 border-b border-slate-100 px-2 py-2 text-xs last:border-b-0"
                key={`${redirect.status}-${redirect.from}-${index}`}
              >
                <span className="font-semibold text-slate-700">
                  {redirect.status}
                </span>
                <span className="break-words text-slate-600">{redirect.from}</span>
                <span className="break-words text-slate-900">{redirect.to}</span>
              </div>
            ))}
          </div>
        </div>
      ) : null}

      <div className="grid min-h-0 min-w-0 gap-3 lg:grid-cols-[minmax(220px,0.45fr)_minmax(0,1fr)]">
        <div className="min-h-0 min-w-0 overflow-auto rounded-md border border-slate-200">
          <table className="w-full table-fixed border-collapse text-left text-xs">
            <thead>
              <tr className="border-b border-slate-200 bg-slate-50 text-slate-600">
                <th className="w-40 px-2 py-2 font-semibold">{t("response.headers")}</th>
                <th className="px-2 py-2 font-semibold">{t("fields.value")}</th>
              </tr>
            </thead>
            <tbody>
              {execution.headers.map((header, index) => (
                <tr className="border-b border-slate-100" key={`${header.name}-${index}`}>
                  <td className="break-words px-2 py-2 font-medium text-slate-700">
                    {header.name}
                  </td>
                  <td className="break-words px-2 py-2 text-slate-600">
                    {header.value}
                  </td>
                </tr>
              ))}
              {execution.headers.length === 0 ? (
                <tr>
                  <td className="px-2 py-5 text-center text-slate-500" colSpan={2}>
                    {t("response.noHeaders")}
                  </td>
                </tr>
              ) : null}
            </tbody>
          </table>
        </div>
        <ResponseBodyViewer execution={execution} viewer={viewer} />
      </div>
    </section>
  );
}

function ResponseBodyViewer({
  execution,
  viewer,
}: {
  execution: ResponseExecutionState;
  viewer: ReturnType<typeof createResponseViewerModel>;
}) {
  const { formatError, formatNumber, t } = useI18n();
  const [mode, setMode] = useState<BodyViewMode>("pretty");
  const [search, setSearch] = useState("");
  const [structured, setStructured] = useState<StructuredViewerResult | null>(null);
  const [saveMessage, setSaveMessage] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    if (viewer.kind !== "json" && viewer.kind !== "xml") {
      setStructured(null);
      return;
    }
    setStructured(null);
    void prepareStructuredViewerAsync({
      kind: viewer.kind,
      raw: viewer.rawPreview,
      search,
    }).then((result) => {
      if (!cancelled) {
        setStructured(result);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [viewer.kind, viewer.rawPreview, search]);

  useEffect(() => {
    setMode(viewer.kind === "html" || viewer.kind === "svg" ? "preview" : "pretty");
    setSearch("");
    setSaveMessage(null);
  }, [viewer.kind, viewer.rawPreview]);

  async function onSaveResponseFile() {
    if (!execution.responseFile) {
      return;
    }
    const destinationPath = window.prompt(
      "Save response file to absolute path",
      execution.responseFile.path.replace(/\.tmp$/, ""),
    );
    if (!destinationPath) {
      return;
    }
    try {
      const result = await saveResponseFile({
        sourcePath: execution.responseFile.path,
        destinationPath,
      });
      setSaveMessage(t("response.saved", {
        value: formatNumber(result.byteCount),
        destination: result.destinationPath,
      }));
    } catch (error) {
      setSaveMessage(formatError(error));
    }
  }

  const isStructured = viewer.kind === "json" || viewer.kind === "xml";
  const canPreview = viewer.kind === "html" || viewer.kind === "svg";
  const rawText = formatBodyPreview(viewer.rawPreview);
  const prettyText = structured?.pretty ?? rawText;

  return (
    <div className="grid min-h-0 min-w-0 gap-2 rounded-md border border-slate-700 bg-slate-950 p-3 text-slate-50">
      <div className="flex min-w-0 flex-wrap items-center gap-2">
        <span className="rounded border border-slate-700 px-2 py-1 text-xs font-semibold">
          {viewer.displayName}
        </span>
        {viewer.contentType ? (
          <span className="text-xs text-slate-300">Type {viewer.contentType}</span>
        ) : null}
        {viewer.charset ? (
          <span className="text-xs text-slate-300">charset {viewer.charset}</span>
        ) : null}
        <span className="text-xs text-slate-300">
          decoded {formatByteCount(viewer.decodedBytes)}
        </span>
        <span className="text-xs text-slate-300">hash {viewer.previewHash}</span>
        {execution.responseFile ? (
          <Button
            className="ml-auto h-7 border-slate-600 bg-slate-950 px-2 text-xs text-slate-50 hover:bg-slate-800 hover:text-slate-50"
            onClick={() => void onSaveResponseFile()}
            size="sm"
            type="button"
            variant="outline"
          >
            Save
          </Button>
        ) : null}
      </div>

      <div className="flex min-w-0 flex-wrap items-center gap-2">
        <Button
          aria-pressed={mode === "pretty"}
          className={modeButtonClass(mode === "pretty")}
          onClick={() => setMode("pretty")}
          size="sm"
          type="button"
          variant="outline"
        >
          Pretty
        </Button>
        <Button
          aria-pressed={mode === "raw"}
          className={modeButtonClass(mode === "raw")}
          onClick={() => setMode("raw")}
          size="sm"
          type="button"
          variant="outline"
        >
          Raw
        </Button>
        {canPreview ? (
          <Button
            aria-pressed={mode === "preview"}
            className={modeButtonClass(mode === "preview")}
            onClick={() => setMode("preview")}
            size="sm"
            type="button"
            variant="outline"
          >
            Preview
          </Button>
        ) : null}
        {isStructured ? (
          <>
            <Input
              aria-label={t("response.search")}
              className="h-7 min-w-32 flex-1 border-slate-700 bg-slate-900 text-xs text-slate-50 placeholder:text-slate-400"
              onChange={(event) => setSearch(event.target.value)}
              placeholder={t("response.search")}
              value={search}
            />
            <span className="text-xs text-slate-300">
              {structured?.matchCount ?? 0} matches
            </span>
          </>
        ) : null}
      </div>

      {structured?.error ? (
        <p className="rounded border border-amber-300 bg-amber-50 px-2 py-1 text-xs font-semibold text-amber-900">
          {structured.error}
        </p>
      ) : null}

      <ResponseBodyContent
        mode={mode}
        prettyText={prettyText}
        rawText={rawText}
        viewerKind={viewer.kind}
      />

      {viewer.bodyTruncated ? (
        <p className="text-xs text-amber-200">{t("response.truncated")}</p>
      ) : null}
      {execution.responseFile ? (
        <p className="break-words text-xs text-slate-300">
          Temporary response file: {execution.responseFile.path}
        </p>
      ) : null}
      {viewer.kind === "image" || viewer.kind === "binary" ? (
        <p className="text-xs text-slate-300">
          Rendering skipped for untrusted bytes. Response file size{" "}
          {formatByteCount(viewer.responseFileBytes)}.
        </p>
      ) : null}
      {saveMessage ? (
        <p aria-live="polite" className="break-words text-xs text-slate-300">
          {saveMessage}
        </p>
      ) : null}
    </div>
  );
}

function ResponseBodyContent({
  mode,
  prettyText,
  rawText,
  viewerKind,
}: {
  mode: BodyViewMode;
  prettyText: string;
  rawText: string;
  viewerKind: ReturnType<typeof createResponseViewerModel>["kind"];
}) {
  const { t } = useI18n();
  if (viewerKind === "empty") {
    return <p className="py-8 text-center text-sm text-slate-300">{t("response.noBody")}</p>;
  }
  if (mode === "preview" && viewerKind === "html") {
    return (
      <iframe
        className="h-52 w-full rounded border border-slate-700 bg-white"
        sandbox=""
        srcDoc={htmlSandboxSource(rawText)}
        title="Sandboxed HTML response preview"
      />
    );
  }
  if (mode === "preview" && viewerKind === "svg") {
    return (
      <iframe
        className="h-52 w-full rounded border border-slate-700 bg-white"
        sandbox=""
        srcDoc={svgSandboxSource(rawText)}
        title="Sandboxed SVG response preview"
      />
    );
  }

  return (
    <pre className="max-h-52 overflow-auto whitespace-pre-wrap break-words text-xs leading-5">
      {mode === "raw" ? rawText : prettyText || t("response.noBody")}
    </pre>
  );
}

function modeButtonClass(active: boolean) {
  return [
    "h-7 px-2 text-xs font-semibold",
    active
      ? "border-slate-100 bg-slate-100 text-slate-950 hover:bg-slate-200 hover:text-slate-950"
      : "border-slate-700 bg-slate-950 text-slate-200 hover:bg-slate-800 hover:text-slate-50",
  ].join(" ");
}

function formatTiming(execution: ResponseExecutionState) {
  const timing = execution.timing;
  const timingParts: Array<[string, bigint | null]> = [
    ["queue", timing.queuedMs],
    ["dns", timing.dnsMs],
    ["connect", timing.connectMs],
    ["tls", timing.tlsMs],
    ["first byte", timing.firstByteMs],
    ["download", timing.downloadMs],
    ["total", timing.totalMs],
  ];
  const parts = timingParts
    .filter((entry): entry is [string, bigint] => entry[1] !== null)
    .map(([label, value]) => `${label} ${value.toString()} ms`);

  return parts.length > 0 ? parts.join(" / ") : "pending";
}
