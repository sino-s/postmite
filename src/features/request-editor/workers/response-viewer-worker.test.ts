import { describe, expect, it } from "vitest";

import { prepareStructuredViewer } from "./response-viewer-worker-core";

describe("response viewer worker core", () => {
  it("formats JSON and counts search matches off the component path", () => {
    const result = prepareStructuredViewer({
      kind: "json",
      raw: "{\"ok\":true,\"items\":[{\"ok\":false}]}",
      search: "ok",
    });

    expect(result.error).toBeNull();
    expect(result.pretty).toContain('"items": [');
    expect(result.matchCount).toBe(2);
  });

  it("falls back to raw text for malformed structured previews", () => {
    const result = prepareStructuredViewer({
      kind: "json",
      raw: "{\"ok\":",
      search: "ok",
    });

    expect(result.pretty).toBe("{\"ok\":");
    expect(result.error).toContain("Invalid JSON");
    expect(result.matchCount).toBe(1);
  });

  it("formats XML without executing document content", () => {
    const result = prepareStructuredViewer({
      kind: "xml",
      raw: "<root><item>one</item><item>two</item></root>",
      search: "item",
    });

    expect(result.error).toBeNull();
    expect(result.pretty).toContain("\n  <item>");
    expect(result.matchCount).toBe(4);
  });
});
