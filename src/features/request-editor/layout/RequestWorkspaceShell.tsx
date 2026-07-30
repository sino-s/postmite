import type { ReactNode } from "react";

import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "../../../components/ui/resizable";
import { useMediaQuery } from "../hooks/useMediaQuery";

type RequestWorkspaceShellProps = {
  editorPane: ReactNode;
  sidebar: ReactNode;
};

export function RequestWorkspaceShell({
  editorPane,
  sidebar,
}: RequestWorkspaceShellProps) {
  const isDesktopLayout = useMediaQuery("(min-width: 768px)", true);

  if (!isDesktopLayout) {
    return (
      <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
        <div className="max-h-[34vh] min-h-0 shrink-0 overflow-hidden [&>aside]:h-full">
          {sidebar}
        </div>
        <div className="min-h-0 flex-1 overflow-hidden">
          {editorPane}
        </div>
      </div>
    );
  }

  return (
    <ResizablePanelGroup className="min-h-0 flex-1" orientation="horizontal">
      <ResizablePanel
        className="overflow-hidden"
        defaultSize="24"
        maxSize="36"
        minSize="220px"
      >
        {sidebar}
      </ResizablePanel>
      <ResizableHandle aria-label="Resize collections and request workspace" withHandle />
      <ResizablePanel className="overflow-hidden" minSize="50">
        {editorPane}
      </ResizablePanel>
    </ResizablePanelGroup>
  );
}
