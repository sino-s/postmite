import { useState } from "react";

import { Button } from "../../components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../../components/ui/dialog";
import { Input } from "../../components/ui/input";
import { NativeSelect } from "../../components/ui/native-select";
import type { WorkspaceSummaryDto } from "../../shared/api/generated/ipc";
import { useI18n } from "../../app/i18n";

type WorkspaceManagerDialogProps = {
  onCreate: (name: string) => Promise<void>;
  onDelete: (workspaceId: string) => Promise<void>;
  onOpenChange: (open: boolean) => void;
  onRename: (workspaceId: string, name: string) => Promise<void>;
  onSelect: (workspaceId: string) => Promise<void>;
  open: boolean;
  selectedWorkspaceId: string;
  workspaces: WorkspaceSummaryDto[];
};

export function WorkspaceManagerDialog(props: WorkspaceManagerDialogProps) {
  if (!props.open) {
    return null;
  }
  return <WorkspaceManagerForm {...props} />;
}

function WorkspaceManagerForm({
  onCreate,
  onDelete,
  onOpenChange,
  onRename,
  onSelect,
  open,
  selectedWorkspaceId,
  workspaces,
}: WorkspaceManagerDialogProps) {
  const { formatError, t } = useI18n();
  const selected =
    workspaces.find((workspace) => workspace.id === selectedWorkspaceId) ??
    workspaces[0];
  const [managedId, setManagedId] = useState(selected?.id ?? "");
  const managed = workspaces.find((workspace) => workspace.id === managedId) ?? selected;
  const [renameValue, setRenameValue] = useState(managed?.name ?? "");
  const [createValue, setCreateValue] = useState("");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function run(operation: () => Promise<void>) {
    setPending(true);
    setError(null);
    try {
      await operation();
    } catch (caught) {
      setError(formatError(caught));
    } finally {
      setPending(false);
    }
  }

  return (
    <Dialog onOpenChange={onOpenChange} open={open}>
      <DialogContent aria-describedby="workspace-manager-description">
        <DialogHeader>
          <DialogTitle>{t("workspace.title")}</DialogTitle>
          <DialogDescription id="workspace-manager-description">
            {t("workspace.description")}
          </DialogDescription>
        </DialogHeader>

        <div className="grid gap-4">
          <label className="grid gap-1 text-sm font-medium" htmlFor="managed-workspace">
            {t("workspace.current")}
            <NativeSelect
              id="managed-workspace"
              onChange={(event) => {
                const id = event.currentTarget.value;
                const workspace = workspaces.find((candidate) => candidate.id === id);
                setManagedId(id);
                setRenameValue(workspace?.name ?? "");
                void run(() => onSelect(id));
              }}
              value={managedId}
            >
              {workspaces.map((workspace) => (
                <option key={workspace.id} value={workspace.id}>
                  {workspace.name}
                </option>
              ))}
            </NativeSelect>
          </label>

          <form
            className="flex gap-2"
            onSubmit={(event) => {
              event.preventDefault();
              const name = renameValue.trim();
              if (managed && name && name !== managed.name) {
                void run(() => onRename(managed.id, name));
              }
            }}
          >
            <Input
              aria-label={t("workspace.rename")}
              disabled={pending}
              onChange={(event) => setRenameValue(event.currentTarget.value)}
              value={renameValue}
            />
            <Button disabled={pending || !renameValue.trim()} type="submit" variant="outline">
              {t("workspace.rename")}
            </Button>
          </form>

          <form
            className="flex gap-2"
            onSubmit={(event) => {
              event.preventDefault();
              const name = createValue.trim();
              if (name) {
                void run(async () => {
                  await onCreate(name);
                  setCreateValue("");
                });
              }
            }}
          >
            <Input
              aria-label={t("workspace.newName")}
              disabled={pending}
              onChange={(event) => setCreateValue(event.currentTarget.value)}
              placeholder={t("workspace.newName")}
              value={createValue}
            />
            <Button disabled={pending || !createValue.trim()} type="submit">
              {t("workspace.create")}
            </Button>
          </form>

          {error ? <p className="text-sm text-destructive" role="alert">{error}</p> : null}
        </div>

        <DialogFooter>
          <Button
            disabled={pending || workspaces.length <= 1 || !managed}
            onClick={() => {
              if (
                managed &&
                window.confirm(t("workspace.deleteConfirm", { name: managed.name }))
              ) {
                void run(() => onDelete(managed.id));
              }
            }}
            type="button"
            variant="destructive"
          >
            {t("workspace.delete")}
          </Button>
          <Button onClick={() => onOpenChange(false)} type="button" variant="outline">
            {t("common.close")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
