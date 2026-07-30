import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { ResponsePanel } from "./ResponsePanel";
import type { ResponseExecutionState } from "../../../shared/api/execution";

const saveResponseFileMock = vi.hoisted(() => vi.fn());

vi.mock("../../../shared/api/execution", async (importActual) => {
  const actual =
    await importActual<typeof import("../../../shared/api/execution")>();
  return {
    ...actual,
    saveResponseFile: saveResponseFileMock,
  };
});

describe("ResponsePanel", () => {
  beforeEach(() => {
    saveResponseFileMock.mockReset();
  });

  it("renders JSON pretty view with search results from the worker path", async () => {
    const user = userEvent.setup();
    render(
      <ResponsePanel
        execution={execution({
          headers: [{ name: "content-type", value: "application/json" }],
          bodyPreview: "{\"ok\":true}",
          decodedBytes: 11n,
        })}
      />,
    );

    expect(await screen.findByText(/"ok": true/)).toBeInTheDocument();
    await user.type(screen.getByLabelText("Search response"), "ok");
    expect(await screen.findByText("1 matches")).toBeInTheDocument();
    expect(screen.getByText("ok").tagName).toBe("MARK");
  });

  it("switches between body and headers without rendering them side by side", async () => {
    const user = userEvent.setup();
    render(
      <ResponsePanel
        execution={execution({
          headers: [{ name: "x-long-header", value: "long readable value" }],
          bodyPreview: "response body",
          decodedBytes: 13n,
        })}
      />,
    );

    expect(screen.getByRole("tab", { name: "Body" })).toHaveAttribute("aria-selected", "true");
    expect(screen.queryByText("x-long-header")).not.toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: "Headers" }));

    expect(screen.getByRole("tab", { name: "Headers" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByText("x-long-header")).toBeInTheDocument();
    expect(screen.queryByText("response body")).not.toBeInTheDocument();
  });

  it("hides raw mode when it would match the pretty response", async () => {
    render(
      <ResponsePanel
        execution={execution({
          headers: [{ name: "content-type", value: "application/json" }],
          bodyPreview: "{}",
          decodedBytes: 2n,
        })}
      />,
    );

    expect(await screen.findByText("{}")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Raw" })).not.toBeInTheDocument();
  });

  it("uses a scriptless sandbox for HTML previews", async () => {
    render(
      <ResponsePanel
        execution={execution({
          headers: [{ name: "content-type", value: "text/html" }],
          bodyPreview:
            "<script>window.__postmiteEscape=1</script><p onclick=\"x()\">ok</p>",
        })}
      />,
    );

    const frame = screen.getByTitle("Sandboxed HTML response preview");
    expect(frame).toHaveAttribute("sandbox", "");
    expect(frame).toHaveAttribute("srcdoc", expect.not.stringContaining("<script"));
    expect(frame).toHaveAttribute("srcdoc", expect.not.stringContaining("onclick"));
  });

  it("exposes save for binary response files through typed IPC", async () => {
    const user = userEvent.setup();
    const prompt = vi
      .spyOn(window, "prompt")
      .mockReturnValue("/home/sino/downloads/fixture.bin");
    saveResponseFileMock.mockResolvedValue({
      destinationPath: "/home/sino/downloads/fixture.bin",
      byteCount: 4096n,
    });

    render(
      <ResponsePanel
        execution={execution({
          headers: [{ name: "content-type", value: "application/octet-stream" }],
          bodyPreview: "\u0000\u0001preview",
          decodedBytes: 4096n,
          bodyTruncated: true,
          responseFile: {
            path: "/tmp/postmite-response-files/response.fixture.tmp",
            byteCount: 4096n,
            expiresAtEpochSeconds: 1_800_086_400n,
          },
        })}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(saveResponseFileMock).toHaveBeenCalledWith({
      sourcePath: "/tmp/postmite-response-files/response.fixture.tmp",
      destinationPath: "/home/sino/downloads/fixture.bin",
    });
    await waitFor(() =>
      expect(screen.getByText(/Saved 4096 bytes/)).toBeInTheDocument(),
    );
    prompt.mockRestore();
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
