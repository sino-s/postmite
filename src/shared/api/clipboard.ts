import { writeText } from "@tauri-apps/plugin-clipboard-manager";

export async function writeClipboardText(value: string) {
  await writeText(value);
}
