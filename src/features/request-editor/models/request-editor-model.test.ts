import { describe, expect, it } from "vitest";

import type {
  RequestBodyDto,
  RequestContentDto,
  RequestDraftDto,
} from "../../../shared/api/generated/ipc";
import {
  bodyModeLabel,
  emptyBodyForMode,
  emptyRequestContent,
  formatBodyPreview,
  isDraftDirty,
  requestContentQueryKey,
  type OverrideMap,
} from "./request-editor-model";

describe("request editor model", () => {
  it("keeps the current body when switching to the existing mode", () => {
    const body: RequestBodyDto = { type: "RAW", content: "{\"ok\":true}" };

    expect(emptyBodyForMode("RAW", body)).toBe(body);
  });

  it("creates stable empty body defaults for each editor mode", () => {
    expect(emptyBodyForMode("NONE", { type: "RAW", content: "body" })).toEqual({
      type: "NONE",
    });
    expect(emptyBodyForMode("URL_ENCODED", { type: "NONE" })).toEqual({
      type: "URL_ENCODED",
      fields: [],
    });
    expect(emptyBodyForMode("MULTIPART", { type: "NONE" })).toEqual({
      type: "MULTIPART",
      parts: [],
    });
    expect(emptyBodyForMode("BINARY", { type: "NONE" })).toEqual({
      type: "BINARY",
      file: {
        path: { type: "RELATIVE", path: "" },
        fileName: "",
        size: 0n,
        modifiedAtEpochSeconds: null,
        sha256: "",
      },
    });
  });

  it("preserves raw text when switching from raw to raw-compatible mode defaults", () => {
    expect(emptyBodyForMode("RAW", { type: "NONE" })).toEqual({
      type: "RAW",
      content: "",
    });
    expect(emptyBodyForMode("RAW", { type: "RAW", content: "saved" })).toEqual({
      type: "RAW",
      content: "saved",
    });
  });

  it("serializes request content with bigint values in query keys", () => {
    const content: RequestContentDto = emptyRequestContent();

    expect(requestContentQueryKey(content)).toContain('"connectMs":"10000"');
    expect(requestContentQueryKey(content)).toContain('"overallMs":"300000"');
  });

  it("uses persisted dirty flags and draft overrides for tab dirty state", () => {
    const drafts: RequestDraftDto[] = [
      {
        id: "draft-clean",
        workspaceId: "workspace-1",
        savedRequestId: null,
        content: emptyRequestContent(),
        isDirty: false,
      },
      {
        id: "draft-dirty",
        workspaceId: "workspace-1",
        savedRequestId: null,
        content: emptyRequestContent(),
        isDirty: true,
      },
    ];
    const overrides: OverrideMap = {
      "draft-clean": {
        ...emptyRequestContent(),
        name: "Local override",
      },
    };

    expect(isDraftDirty("draft-clean", drafts, overrides)).toBe(true);
    expect(isDraftDirty("draft-dirty", drafts, {})).toBe(true);
    expect(isDraftDirty("missing", drafts, overrides)).toBe(false);
  });

  it("formats JSON response previews without altering plain text", () => {
    expect(formatBodyPreview('{"ok":true}')).toBe('{\n  "ok": true\n}');
    expect(formatBodyPreview("not-json")).toBe("not-json");
    expect(formatBodyPreview("   ")).toBe("");
  });

  it("labels body modes for segmented controls", () => {
    expect(bodyModeLabel("NONE")).toBe("None");
    expect(bodyModeLabel("RAW")).toBe("Raw");
    expect(bodyModeLabel("URL_ENCODED")).toBe("Form");
    expect(bodyModeLabel("MULTIPART")).toBe("Multipart");
    expect(bodyModeLabel("BINARY")).toBe("Binary");
  });
});
