import { describe, expect, it } from "vitest";

import {
  classifyViewerKind,
  createResponseViewerModel,
  htmlSandboxSource,
  svgSandboxSource,
} from "./response-viewer-model";
import type { ResponseExecutionState } from "../../../shared/api/execution";

describe("response viewer model", () => {
  it("selects structured and media-safe viewers from attacker-controlled MIME types", () => {
    expect(
      classifyViewerKind({
        contentType: "application/json",
        rawPreview: "{\"ok\":true}",
        hasBody: true,
        responseFile: false,
      }),
    ).toBe("json");
    expect(
      classifyViewerKind({
        contentType: "image/svg+xml",
        rawPreview: "<svg></svg>",
        hasBody: true,
        responseFile: false,
      }),
    ).toBe("svg");
    expect(
      classifyViewerKind({
        contentType: "application/octet-stream",
        rawPreview: "\u0000binary",
        hasBody: true,
        responseFile: true,
      }),
    ).toBe("binary");
  });

  it("normalizes content metadata without exposing response file bytes as text", () => {
    const model = createResponseViewerModel(
      execution({
        headers: [{ name: "Content-Type", value: "text/html; charset=utf-8" }],
        bodyPreview: "<html><body>ok</body></html>",
        decodedBytes: 128n,
        responseFile: {
          path: "/tmp/postmite-response-files/response.fixture.tmp",
          byteCount: 128n,
          expiresAtEpochSeconds: 1_800_086_400n,
        },
      }),
    );

    expect(model).toMatchObject({
      kind: "html",
      contentType: "text/html",
      charset: "utf-8",
      decodedBytes: 128n,
      responseFileBytes: 128n,
      canSave: true,
    });
    expect(model.previewHash).toMatch(/^[0-9a-f]{8}$/);
  });

  it("builds scriptless HTML and SVG sandbox documents", () => {
    const html = htmlSandboxSource(
      "<script>window.__x=1</script><img src=\"https://example.test/x\"><p onclick=\"x()\">ok</p>",
    );
    const svg = svgSandboxSource(
      "<svg><script>alert(1)</script><foreignObject><body>x</body></foreignObject><a href=\"https://example.test\">x</a></svg>",
    );

    expect(html).toContain("Content-Security-Policy");
    expect(html).not.toContain("<script");
    expect(html).not.toContain("onclick");
    expect(html).not.toContain("https://example.test/x");
    expect(svg).not.toContain("<script");
    expect(svg).not.toContain("foreignObject");
    expect(svg).not.toContain("https://example.test");
  });
});

function execution(
  overrides: Partial<ResponseExecutionState> = {},
): ResponseExecutionState {
  return {
    draftId: "draft-1",
    executionId: "execution-1",
    phase: "completed",
    startedAtMs: 100,
    completedAtMs: 120,
    lastSequence: 1n,
    method: "GET",
    url: "https://example.test",
    tlsVerification: true,
    proxy: null,
    timeouts: null,
    timing: {
      queuedMs: 0n,
      dnsMs: null,
      connectMs: null,
      tlsMs: null,
      firstByteMs: null,
      downloadMs: null,
      totalMs: 20n,
    },
    redirects: [],
    status: 200,
    protocol: "HTTP/2",
    remoteAddr: null,
    headers: [],
    bodyPreview: "",
    bodyTruncated: false,
    decodedBytes: null,
    wireBytes: null,
    responseFile: null,
    error: null,
    uploadProgress: null,
    downloadProgress: null,
    ...overrides,
  };
}
