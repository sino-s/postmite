import { useEffect, useState } from "react";

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

type ResponsePanelProps = {
  execution: ResponseExecutionState | null;
};

type BodyViewMode = "pretty" | "raw" | "preview";

export function ResponsePanel({ execution }: ResponsePanelProps) {
  if (!execution) {
    return (
      <section
        aria-label="Response"
        aria-live="polite"
        className="min-h-40 rounded-md border border-slate-300 bg-white p-3 text-sm text-slate-600"
      >
        No response yet.
      </section>
    );
  }

  const elapsedMs =
    (execution.completedAtMs ?? Date.now()) - execution.startedAtMs;
  const phaseLabel = execution.phase[0].toUpperCase() + execution.phase.slice(1);
  const viewer = createResponseViewerModel(execution);

  return (
    <section
      aria-label="Response"
      aria-live="polite"
      className="grid min-h-40 gap-3 rounded-md border border-slate-300 bg-white p-3 text-sm"
    >
      <div className="flex flex-wrap items-center gap-3">
        <span className="rounded-md border border-slate-300 px-2 py-1 font-medium text-slate-700">
          {phaseLabel}
        </span>
        {execution.status ? (
          <span className="font-semibold text-slate-950">
            Status {execution.status}
          </span>
        ) : null}
        <span className="text-slate-600">Time {Math.max(0, elapsedMs)} ms</span>
        {execution.protocol ? (
          <span className="text-slate-600">{execution.protocol}</span>
        ) : null}
        <span className="text-slate-600">
          Timing {formatTiming(execution)}
        </span>
        {execution.downloadProgress ? (
          <span className="text-slate-600">
            Received {execution.downloadProgress.receivedBytes.toString()} bytes
          </span>
        ) : null}
        {execution.decodedBytes !== null ? (
          <span className="text-slate-600">
            Decoded {execution.decodedBytes.toString()} bytes
          </span>
        ) : null}
        {execution.wireBytes !== null ? (
          <span className="text-slate-600">
            Wire {execution.wireBytes.toString()} bytes
          </span>
        ) : null}
        {execution.responseFile ? (
          <span className="text-slate-600">
            File {execution.responseFile.byteCount.toString()} bytes
          </span>
        ) : null}
        {execution.uploadProgress ? (
          <span className="text-slate-600">
            Sent {execution.uploadProgress.sentBytes.toString()} bytes
          </span>
        ) : null}
        {execution.proxy ? (
          <span className="text-slate-600">
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

      <div className="grid min-h-0 gap-3 lg:grid-cols-[minmax(260px,0.45fr)_minmax(0,1fr)]">
        <div className="min-h-0 overflow-auto rounded-md border border-slate-200">
          <table className="w-full table-fixed border-collapse text-left text-xs">
            <thead>
              <tr className="border-b border-slate-200 bg-slate-50 text-slate-600">
                <th className="w-40 px-2 py-2 font-semibold">Header</th>
                <th className="px-2 py-2 font-semibold">Value</th>
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
                    No response headers
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
      setSaveMessage(
        `Saved ${result.byteCount.toString()} bytes to ${result.destinationPath}`,
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : "Save failed.";
      setSaveMessage(message);
    }
  }

  const isStructured = viewer.kind === "json" || viewer.kind === "xml";
  const canPreview = viewer.kind === "html" || viewer.kind === "svg";
  const rawText = formatBodyPreview(viewer.rawPreview);
  const prettyText = structured?.pretty ?? rawText;

  return (
    <div className="grid min-h-0 gap-2 rounded-md border border-slate-200 bg-slate-950 p-3 text-slate-50">
      <div className="flex flex-wrap items-center gap-2">
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
          <button
            className="ml-auto rounded border border-slate-600 px-2 py-1 text-xs font-semibold text-slate-50 hover:bg-slate-800"
            onClick={() => void onSaveResponseFile()}
            type="button"
          >
            Save
          </button>
        ) : null}
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <button
          aria-pressed={mode === "pretty"}
          className={modeButtonClass(mode === "pretty")}
          onClick={() => setMode("pretty")}
          type="button"
        >
          Pretty
        </button>
        <button
          aria-pressed={mode === "raw"}
          className={modeButtonClass(mode === "raw")}
          onClick={() => setMode("raw")}
          type="button"
        >
          Raw
        </button>
        {canPreview ? (
          <button
            aria-pressed={mode === "preview"}
            className={modeButtonClass(mode === "preview")}
            onClick={() => setMode("preview")}
            type="button"
          >
            Preview
          </button>
        ) : null}
        {isStructured ? (
          <>
            <input
              aria-label="Search response"
              className="min-w-0 flex-1 rounded border border-slate-700 bg-slate-900 px-2 py-1 text-xs text-slate-50"
              onChange={(event) => setSearch(event.target.value)}
              placeholder="Search"
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
        <p className="text-xs text-amber-200">Response preview truncated.</p>
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
  if (viewerKind === "empty") {
    return <p className="py-8 text-center text-sm text-slate-300">No response body</p>;
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
      {mode === "raw" ? rawText : prettyText || "No response body"}
    </pre>
  );
}

function modeButtonClass(active: boolean) {
  return [
    "rounded border px-2 py-1 text-xs font-semibold",
    active
      ? "border-slate-100 bg-slate-100 text-slate-950"
      : "border-slate-700 text-slate-200 hover:bg-slate-800",
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
