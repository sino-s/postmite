import { GripVertical } from "lucide-react";
import type { ComponentPropsWithoutRef } from "react";
import * as ResizablePrimitive from "react-resizable-panels";

import { cn } from "../../shared/lib/utils";

const ResizablePanelGroup = ({
  className,
  orientation = "horizontal",
  ...props
}: ComponentPropsWithoutRef<typeof ResizablePrimitive.Group>) => (
  <ResizablePrimitive.Group
    className={cn(
      "flex h-full w-full",
      orientation === "vertical" && "flex-col",
      className,
    )}
    orientation={orientation}
    {...props}
  />
);

const ResizablePanel = ResizablePrimitive.Panel;

const ResizableHandle = ({
  className,
  orientation = "horizontal",
  withHandle,
  ...props
}: ComponentPropsWithoutRef<typeof ResizablePrimitive.Separator> & {
  orientation?: "horizontal" | "vertical";
  withHandle?: boolean;
}) => (
  <ResizablePrimitive.Separator
    className={cn(
      "relative flex items-center justify-center bg-border focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring",
      orientation === "horizontal" &&
        "w-px after:absolute after:inset-y-0 after:left-1/2 after:w-1 after:-translate-x-1/2",
      orientation === "vertical" &&
        "h-px w-full after:absolute after:left-0 after:h-1 after:w-full after:translate-x-0 after:-translate-y-1/2",
      className,
    )}
    {...props}
  >
    {withHandle ? (
      <div className="z-10 flex h-4 w-3 items-center justify-center rounded-sm border border-border bg-background">
        <GripVertical aria-hidden="true" className="h-3 w-3" />
      </div>
    ) : null}
  </ResizablePrimitive.Separator>
);

export { ResizableHandle, ResizablePanel, ResizablePanelGroup };
