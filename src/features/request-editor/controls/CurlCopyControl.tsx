import { Copy } from "lucide-react";

import { useI18n } from "../../../app/i18n";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "../../../components/ui/alert-dialog";
import { Button } from "../../../components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "../../../components/ui/tooltip";

export type CurlCopyFeedback = "copied" | "failed" | "stale" | null;

type CurlCopyControlProps = {
  confirmationOpen: boolean;
  disabled: boolean;
  feedback: CurlCopyFeedback;
  onCancelConfirmation: () => void;
  onCopy: () => void;
  onCopyRedacted: () => void;
  onIncludeSecrets: () => void;
  pending: boolean;
};

export function CurlCopyControl({
  confirmationOpen,
  disabled,
  feedback,
  onCancelConfirmation,
  onCopy,
  onCopyRedacted,
  onIncludeSecrets,
  pending,
}: CurlCopyControlProps) {
  const { t } = useI18n();
  const disabledReason = pending
    ? t("curl.copyPending")
    : t("curl.copyUnavailable");
  const tooltip = disabled ? disabledReason : t("curl.copyTooltip");

  return (
    <>
      <div className="relative flex shrink-0 items-center gap-2">
        <TooltipProvider delayDuration={0}>
          <Tooltip>
            <TooltipTrigger asChild>
              <span className="inline-flex">
                <Button
                  aria-label={t("curl.copy")}
                  className="h-9 gap-1.5 px-2.5 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring"
                  disabled={disabled}
                  onClick={onCopy}
                  type="button"
                  variant="outline"
                >
                  <Copy aria-hidden="true" size={15} />
                  <span>cURL</span>
                </Button>
              </span>
            </TooltipTrigger>
            <TooltipContent>{tooltip}</TooltipContent>
          </Tooltip>
        </TooltipProvider>
        {feedback ? (
          <span
            aria-live={feedback === "failed" ? "assertive" : "polite"}
            className={`absolute right-0 top-full z-20 mt-1 whitespace-nowrap rounded-md border bg-background px-2 py-1 text-xs shadow-sm ${
              feedback === "failed" ? "text-destructive" : "text-foreground"
            }`}
            role={feedback === "failed" ? "alert" : "status"}
          >
            {t(
              feedback === "copied"
                ? "curl.copySuccess"
                : feedback === "failed"
                  ? "curl.copyFailure"
                  : "curl.copyStale",
            )}
          </span>
        ) : null}
      </div>

      <AlertDialog
        onOpenChange={(open) => {
          if (!open) {
            onCancelConfirmation();
          }
        }}
        open={confirmationOpen}
      >
        <AlertDialogContent
          onOpenAutoFocus={(event) => {
            event.preventDefault();
            document.getElementById("curl-copy-redacted")?.focus();
          }}
        >
          <AlertDialogHeader>
            <AlertDialogTitle>{t("curl.secretTitle")}</AlertDialogTitle>
            <AlertDialogDescription>
              {t("curl.secretDescription")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel onClick={onCancelConfirmation}>
              {t("curl.cancel")}
            </AlertDialogCancel>
            <AlertDialogAction
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
              onClick={onIncludeSecrets}
            >
              {t("curl.includeSecrets")}
            </AlertDialogAction>
            <AlertDialogAction
              id="curl-copy-redacted"
              onClick={onCopyRedacted}
            >
              {t("curl.copyRedacted")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
