import type { ComponentPropsWithoutRef } from "react";

import { cn } from "../../shared/lib/utils";

function Input({ className, type, ...props }: ComponentPropsWithoutRef<"input">) {
  return (
    <input
      className={cn(
        "flex h-[var(--control-height)] w-full rounded-md border border-input bg-background px-3 py-1 text-sm text-foreground shadow-sm transition-colors file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring disabled:cursor-not-allowed disabled:opacity-50",
        className,
      )}
      type={type}
      {...props}
    />
  );
}

export { Input };
