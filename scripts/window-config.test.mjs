import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const repoRoot = resolve(process.cwd());

describe("main Tauri window", () => {
  it("uses the requested resizable 1600 by 1080 baseline", () => {
    const config = JSON.parse(
      readFileSync(resolve(repoRoot, "src-tauri/tauri.conf.json"), "utf8"),
    );
    const mainWindow = config.app.windows.find(({ label }) => label === "main");

    expect(mainWindow).toMatchObject({
      width: 1600,
      height: 1080,
      minWidth: 1600,
      minHeight: 1080,
      resizable: true,
      fullscreen: false,
    });
  });

  it("restores only native window size through Rust", () => {
    const manifest = readFileSync(resolve(repoRoot, "src-tauri/Cargo.toml"), "utf8");
    const application = readFileSync(resolve(repoRoot, "src-tauri/src/lib.rs"), "utf8");

    expect(manifest).toContain('tauri-plugin-window-state = "2"');
    expect(application).toContain(".with_state_flags(StateFlags::SIZE)");
    expect(application).not.toContain("StateFlags::all()");
  });
});
