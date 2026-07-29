import type { ResponseExecutionState } from "../../shared/api/execution";

export type ResponseViewerKind =
  | "empty"
  | "json"
  | "xml"
  | "html"
  | "svg"
  | "image"
  | "text"
  | "binary";

export type ResponseViewerModel = {
  kind: ResponseViewerKind;
  contentType: string | null;
  charset: string | null;
  displayName: string;
  rawPreview: string;
  bodyTruncated: boolean;
  decodedBytes: bigint | null;
  responseFileBytes: bigint | null;
  previewHash: string;
  canSave: boolean;
};

export function createResponseViewerModel(
  execution: ResponseExecutionState,
): ResponseViewerModel {
  const contentTypeHeader = headerValue(execution.headers, "content-type");
  const parsedType = parseContentType(contentTypeHeader);
  const contentType = parsedType.mime;
  const rawPreview = execution.bodyPreview;
  const previewHash = hashPreview(rawPreview);
  const hasBody =
    rawPreview.length > 0 ||
    execution.decodedBytes !== null ||
    execution.responseFile !== null;
  const kind = classifyViewerKind({
    contentType,
    rawPreview,
    hasBody,
    responseFile: execution.responseFile !== null,
  });

  return {
    kind,
    contentType,
    charset: parsedType.charset,
    displayName: displayNameForKind(kind),
    rawPreview,
    bodyTruncated: execution.bodyTruncated,
    decodedBytes: execution.decodedBytes,
    responseFileBytes: execution.responseFile?.byteCount ?? null,
    previewHash,
    canSave: execution.responseFile !== null,
  };
}

export function classifyViewerKind({
  contentType,
  rawPreview,
  hasBody,
  responseFile,
}: {
  contentType: string | null;
  rawPreview: string;
  hasBody: boolean;
  responseFile: boolean;
}): ResponseViewerKind {
  if (!hasBody) {
    return "empty";
  }

  if (contentType) {
    if (contentType === "application/json" || contentType.endsWith("+json")) {
      return "json";
    }
    if (
      contentType === "application/xml" ||
      contentType === "text/xml" ||
      contentType.endsWith("+xml")
    ) {
      return contentType === "image/svg+xml" ? "svg" : "xml";
    }
    if (contentType === "text/html") {
      return "html";
    }
    if (contentType === "image/svg+xml") {
      return "svg";
    }
    if (contentType.startsWith("image/")) {
      return "image";
    }
    if (contentType.startsWith("text/")) {
      return "text";
    }
    if (isKnownBinaryContentType(contentType)) {
      return "binary";
    }
  }

  const trimmed = rawPreview.trimStart();
  if (trimmed.startsWith("{") || trimmed.startsWith("[")) {
    return "json";
  }
  if (trimmed.startsWith("<svg")) {
    return "svg";
  }
  if (trimmed.startsWith("<!doctype html") || trimmed.startsWith("<html")) {
    return "html";
  }
  if (trimmed.startsWith("<")) {
    return "xml";
  }

  return responseFile && looksBinary(rawPreview) ? "binary" : "text";
}

export function htmlSandboxSource(rawHtml: string) {
  return `<!doctype html>
<html>
<head>
<meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src data: blob:; style-src 'unsafe-inline';">
<base href="about:blank">
</head>
<body>${sanitizeHtmlFragment(rawHtml)}</body>
</html>`;
}

export function svgSandboxSource(rawSvg: string) {
  return htmlSandboxSource(sanitizeSvg(rawSvg));
}

export function formatByteCount(value: bigint | null) {
  if (value === null) {
    return "unknown";
  }
  return `${value.toString()} bytes`;
}

function parseContentType(value: string | null) {
  if (!value) {
    return { mime: null, charset: null };
  }
  const [type, ...parameters] = value.split(";");
  const mime = type.trim().toLowerCase() || null;
  const charset =
    parameters
      .map((parameter) => parameter.trim())
      .find((parameter) => parameter.toLowerCase().startsWith("charset="))
      ?.slice("charset=".length)
      .replace(/^"|"$/g, "") ?? null;

  return { mime, charset };
}

function headerValue(
  headers: Array<{ name: string; value: string }>,
  name: string,
) {
  const found = headers.find(
    (header) => header.name.toLowerCase() === name.toLowerCase(),
  );
  return found?.value ?? null;
}

function displayNameForKind(kind: ResponseViewerKind) {
  switch (kind) {
    case "empty":
      return "Empty";
    case "json":
      return "JSON";
    case "xml":
      return "XML";
    case "html":
      return "HTML";
    case "svg":
      return "SVG";
    case "image":
      return "Image";
    case "text":
      return "Text";
    case "binary":
      return "Binary";
  }
}

function isKnownBinaryContentType(contentType: string) {
  return (
    contentType === "application/octet-stream" ||
    contentType === "application/pdf" ||
    contentType.startsWith("audio/") ||
    contentType.startsWith("video/") ||
    contentType.startsWith("font/")
  );
}

function looksBinary(value: string) {
  if (value.length === 0) {
    return false;
  }
  let controlCharacters = 0;
  for (const character of value.slice(0, 512)) {
    const code = character.charCodeAt(0);
    if ((code >= 0 && code < 9) || (code > 13 && code < 32) || code === 65533) {
      controlCharacters += 1;
    }
  }
  return controlCharacters > 0;
}

function hashPreview(value: string) {
  let hash = 2166136261;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0).toString(16).padStart(8, "0");
}

function sanitizeHtmlFragment(rawHtml: string) {
  return rawHtml
    .replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi, "")
    .replace(/\son[a-z]+\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)/gi, "")
    .replace(/\s(?:src|href)\s*=\s*("|')?(?!data:|blob:|#)[^"'\s>]*/gi, "")
    .replace(/<meta\b[^>]*http-equiv\s*=\s*("|')?refresh[\s\S]*?>/gi, "");
}

function sanitizeSvg(rawSvg: string) {
  return sanitizeHtmlFragment(rawSvg)
    .replace(/<foreignObject\b[\s\S]*?<\/foreignObject>/gi, "")
    .replace(/\s(?:xlink:href|href)\s*=\s*("|')?(?!data:|#)[^"'\s>]*/gi, "");
}
