import * as SeparatorPrimitive from "@radix-ui/react-separator";
import type { ComponentPropsWithoutRef } from "react";

import { cn } from "../../shared/lib/utils";

function Separator({
  className,
  decorative = true,
  orientation = "horizontal",
  ...props
}: ComponentPropsWithoutRef<typeof SeparatorPrimitive.Root>) {
  return (
    <SeparatorPrimitive.Root
      className={cn(
        "shrink-0 bg-border",
        orientation === "horizontal" ? "h-px w-full" : "h-full w-px",
        className,
      )}
      decorative={decorative}
      orientation={orientation}
      {...props}
    />
  );
}

export { Separator };
