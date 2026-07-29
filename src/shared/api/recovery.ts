import type { RecoverableDatabaseExportInput } from "./generated/ipc";
import { requestIpc } from "./ipc";

export function getDatabaseRecoveryState() {
  return requestIpc.getDatabaseRecoveryState();
}

export function exportRecoverableDatabase(input: RecoverableDatabaseExportInput) {
  return requestIpc.exportRecoverableDatabase(input);
}
