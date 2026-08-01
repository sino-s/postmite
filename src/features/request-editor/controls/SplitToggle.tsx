import { SquareSplitHorizontal, SquareSplitVertical } from "lucide-react";

import type { RequestResponseSplit } from "../../../app/preferences";
import { Button } from "../../../components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "../../../components/ui/tooltip";

export function SplitToggle({
  split,
  setSplit,
}: {
  split: RequestResponseSplit;
  setSplit: (split: RequestResponseSplit) => void;
}) {
  return (
    <TooltipProvider delayDuration={0}>
      <div
        aria-label="Request and response split"
        className="inline-flex shrink-0 rounded-md border border-input bg-background p-0.5"
        role="group"
      >
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              aria-label="Stack request options above response"
              aria-pressed={split === "horizontal"}
              className={split === "horizontal" ? "bg-accent text-accent-foreground" : ""}
              onClick={() => setSplit("horizontal")}
              size="icon"
              type="button"
              variant="ghost"
            >
              <SquareSplitVertical aria-hidden="true" size={16} />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Stack request options above response</TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              aria-label="Place request options beside response"
              aria-pressed={split === "vertical"}
              className={split === "vertical" ? "bg-accent text-accent-foreground" : ""}
              onClick={() => setSplit("vertical")}
              size="icon"
              type="button"
              variant="ghost"
            >
              <SquareSplitHorizontal aria-hidden="true" size={16} />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Place request options beside response</TooltipContent>
        </Tooltip>
      </div>
    </TooltipProvider>
  );
}
