import type {
  NativeBackupExportInput,
  NativeBackupRestoreInput,
  NativeBackupRestorePreviewInput,
} from "./generated/ipc";
import { requestIpc } from "./ipc";

export function exportNativeBackup(input: NativeBackupExportInput) {
  return requestIpc.exportNativeBackup(input);
}

export function previewNativeBackupRestore(
  input: NativeBackupRestorePreviewInput,
) {
  return requestIpc.previewNativeBackupRestore(input);
}

export function restoreNativeBackup(input: NativeBackupRestoreInput) {
  return requestIpc.restoreNativeBackup(input);
}
