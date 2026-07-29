export type StructuredViewerKind = "json" | "xml";

export type StructuredViewerInput = {
  kind: StructuredViewerKind;
  raw: string;
  search: string;
};

export type StructuredViewerResult = {
  raw: string;
  pretty: string;
  matchCount: number;
  error: string | null;
};

export function prepareStructuredViewer(
  input: StructuredViewerInput,
): StructuredViewerResult {
  const pretty =
    input.kind === "json" ? prettyJson(input.raw) : prettyXml(input.raw);
  return {
    raw: input.raw,
    pretty: pretty.value,
    matchCount: countMatches(pretty.value, input.search),
    error: pretty.error,
  };
}

function prettyJson(raw: string) {
  try {
    return {
      value: JSON.stringify(JSON.parse(raw), null, 2),
      error: null,
    };
  } catch {
    return {
      value: raw,
      error: "Invalid JSON. Showing raw response preview.",
    };
  }
}

function prettyXml(raw: string) {
  const parser = typeof DOMParser === "undefined" ? null : new DOMParser();
  if (parser) {
    const parsed = parser.parseFromString(raw, "application/xml");
    if (parsed.querySelector("parsererror")) {
      return {
        value: raw,
        error: "Invalid XML. Showing raw response preview.",
      };
    }
  }

  const compact = raw.replace(/>\s+</g, "><").trim();
  let depth = 0;
  const lines = compact
    .replace(/></g, ">\n<")
    .split("\n")
    .map((line) => {
      if (/^<\//.test(line)) {
        depth = Math.max(0, depth - 1);
      }
      const formatted = `${"  ".repeat(depth)}${line}`;
      if (/^<[^!?/][^>]*[^/]?>$/.test(line)) {
        depth += 1;
      }
      return formatted;
    });

  return {
    value: lines.join("\n"),
    error: null,
  };
}

function countMatches(value: string, search: string) {
  const needle = search.trim().toLowerCase();
  if (!needle) {
    return 0;
  }
  let count = 0;
  let offset = 0;
  const haystack = value.toLowerCase();
  while (true) {
    const found = haystack.indexOf(needle, offset);
    if (found === -1) {
      return count;
    }
    count += 1;
    offset = found + needle.length;
  }
}
