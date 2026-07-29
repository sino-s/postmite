import type {
  DiagnosticBundleExportInput,
  DiagnosticDebugLoggingInput,
} from "./generated/ipc";
import { requestIpc } from "./ipc";

export function getDiagnosticBundlePreview() {
  return requestIpc.getDiagnosticBundlePreview();
}

export function setDiagnosticDebugLogging(input: DiagnosticDebugLoggingInput) {
  return requestIpc.setDiagnosticDebugLogging(input);
}

export function exportDiagnosticBundle(input: DiagnosticBundleExportInput) {
  return requestIpc.exportDiagnosticBundle(input);
}
