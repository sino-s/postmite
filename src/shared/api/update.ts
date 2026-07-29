import { requestIpc } from "./ipc";

export function checkForUpdate() {
  return requestIpc.checkForUpdate();
}
