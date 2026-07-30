import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const repoRoot = resolve(process.cwd());

describe("Tauri capabilities", () => {
  it("allows the WebView to subscribe to Rust execution events without emit access", () => {
    const capability = JSON.parse(
      readFileSync(
        resolve(repoRoot, "src-tauri/capabilities/main.json"),
        "utf8",
      ),
    );

    expect(capability.permissions).toEqual([
      "core:event:allow-listen",
      "core:event:allow-unlisten",
      "clipboard-manager:allow-write-text",
    ]);
  });
});
