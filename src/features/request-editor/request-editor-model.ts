import type {
  BodyFileReferenceDto,
  CookieSameSiteDto,
  MultipartPartDto,
  RequestBodyDto,
  RequestContentDto,
  RequestDraftDto,
  ResolvedRequestContentDto,
} from "../../shared/api/generated/ipc";

export type OverrideMap = Record<string, RequestContentDto>;

export type CookieFormValue = {
  cookieId: string | null;
  name: string;
  value: string;
  domain: string;
  path: string;
  secure: boolean;
  httpOnly: boolean;
  sameSite: CookieSameSiteDto | null;
  expiresAtEpochSeconds: bigint | null;
};

export function isDraftDirty(
  draftId: string,
  drafts: RequestDraftDto[],
  overrides: OverrideMap,
) {
  const draft = drafts.find((item) => item.id === draftId);
  return Boolean(draft?.isDirty || overrides[draftId]);
}

export function omitKey<T>(record: Record<string, T>, key: string) {
  const next = { ...record };
  delete next[key];
  return next;
}

export function formatBodyPreview(value: string) {
  const trimmed = value.trim();
  if (!trimmed) {
    return "";
  }

  try {
    return JSON.stringify(JSON.parse(trimmed), null, 2);
  } catch {
    return value;
  }
}

export function formatHistoryTime(epochSeconds: bigint) {
  const timestamp = new Date(Number(epochSeconds) * 1000);
  if (Number.isNaN(timestamp.getTime())) {
    return "";
  }
  return timestamp.toLocaleString();
}

export function emptyCookieForm(): CookieFormValue {
  return {
    cookieId: null,
    name: "",
    value: "",
    domain: "",
    path: "/",
    secure: false,
    httpOnly: false,
    sameSite: null,
    expiresAtEpochSeconds: null,
  };
}

export function emptyBodyForMode(
  mode: RequestBodyDto["type"],
  current: RequestBodyDto,
): RequestBodyDto {
  if (current.type === mode) {
    return current;
  }
  switch (mode) {
    case "NONE":
      return { type: "NONE" };
    case "RAW":
      return { type: "RAW", content: bodyToText(current) };
    case "URL_ENCODED":
      return { type: "URL_ENCODED", fields: [] };
    case "MULTIPART":
      return { type: "MULTIPART", parts: [] };
    case "BINARY":
      return { type: "BINARY", file: emptyBodyFileReference() };
  }
}

export function bodyToText(body: RequestBodyDto) {
  return body.type === "RAW" ? body.content : "";
}

export function bodyModeLabel(mode: RequestBodyDto["type"]) {
  switch (mode) {
    case "NONE":
      return "None";
    case "RAW":
      return "Raw";
    case "URL_ENCODED":
      return "Form";
    case "MULTIPART":
      return "Multipart";
    case "BINARY":
      return "Binary";
  }
}

export function emptyMultipartFilePart(order: number): MultipartPartDto {
  return {
    type: "FILE",
    enabled: true,
    order,
    name: "",
    file: emptyBodyFileReference(),
  };
}

export function emptyBodyFileReference(): BodyFileReferenceDto {
  return {
    path: { type: "RELATIVE", path: "" },
    fileName: "",
    size: 0n,
    modifiedAtEpochSeconds: null,
    sha256: "",
  };
}

export function formatSameSite(value: CookieSameSiteDto) {
  switch (value) {
    case "STRICT":
      return "Strict";
    case "LAX":
      return "Lax";
    case "NONE":
      return "None";
  }
}

export function emptyRequestContent(): RequestContentDto {
  return {
    name: "Untitled Request",
    method: "GET",
    url: "",
    body: { type: "NONE" },
    query: [],
    headers: [],
    auth: { type: "NONE" },
    redirect: { enabled: true, maxRedirects: 10 },
    tls: {
      verify: true,
      customCaReference: null,
      clientCertificateReference: null,
      clientKeyReference: null,
    },
    transport: {
      proxy: {
        source: "PROCESS_ENVIRONMENT",
        url: null,
        noProxy: [],
      },
      timeouts: {
        connectMs: 10_000n,
        overallMs: 300_000n,
        idleMs: 60_000n,
      },
    },
  };
}

export function requestContentQueryKey(content: RequestContentDto) {
  return JSON.stringify(content, (_key, value: unknown) =>
    typeof value === "bigint" ? value.toString() : value,
  );
}

export function formatVariableSource(source: string) {
  return source === "ENVIRONMENT" ? "Environment" : "Collection";
}

export function formatProxyMetadata(
  proxy: import("../../shared/api/execution").ResponseExecutionState["proxy"],
) {
  if (!proxy) {
    return "unknown";
  }
  if (proxy.bypassReason) {
    return `${proxy.source} (${proxy.bypassReason})`;
  }
  return proxy.selectedProxy ? `${proxy.source} ${proxy.selectedProxy}` : proxy.source;
}

export function formatResolutionError(kind: string) {
  return kind === "CYCLE" ? "Cyclic reference" : "Missing reference";
}

export function sortResolvedFields(fields: ResolvedRequestContentDto["headers"]) {
  return [...fields].sort((left, right) => left.order - right.order);
}
