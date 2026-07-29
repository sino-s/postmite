import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { DiagnosticsPanel } from "./DiagnosticsPanel";

const diagnosticsApiMock = vi.hoisted(() => ({
  exportDiagnosticBundle: vi.fn(),
  getDiagnosticBundlePreview: vi.fn(),
  setDiagnosticDebugLogging: vi.fn(),
}));

vi.mock("../../../shared/api/diagnostics", () => diagnosticsApiMock);

describe("DiagnosticsPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    diagnosticsApiMock.getDiagnosticBundlePreview.mockResolvedValue({
      entries: ["manifest.json", "runtime-metadata.json", "logs/postmite-diagnostics-0.jsonl"],
      exclusions: ["postmite.sqlite3", "request payloads, URLs, headers, cookies, variables, and Secrets"],
      debugLoggingEnabled: false,
    });
    diagnosticsApiMock.setDiagnosticDebugLogging.mockResolvedValue({
      enabled: true,
      expiresAtEpochSeconds: 1_900_000_000n,
    });
    diagnosticsApiMock.exportDiagnosticBundle.mockResolvedValue({});
  });

  it("reviews excluded content before exporting a diagnostic bundle", async () => {
    const user = userEvent.setup();
    vi.spyOn(window, "prompt").mockReturnValue("/tmp/diagnostics.zip");
    render(<DiagnosticsPanel onClose={vi.fn()} />);

    expect(await screen.findByText("logs/postmite-diagnostics-0.jsonl")).toBeInTheDocument();
    expect(screen.getByText(/Excluded: postmite.sqlite3/)).toBeInTheDocument();
    await user.click(screen.getByLabelText("Temporary debug logging"));
    await user.click(screen.getByRole("button", { name: "Export bundle" }));

    await waitFor(() =>
      expect(diagnosticsApiMock.setDiagnosticDebugLogging).toHaveBeenCalledWith({
        enabled: true,
        durationMinutes: 15,
      }),
    );
    expect(diagnosticsApiMock.exportDiagnosticBundle).toHaveBeenCalledWith({
      bundlePath: "/tmp/diagnostics.zip",
    });
  });
});
