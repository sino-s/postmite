import { ArrowDown, ArrowUp, Plus, Trash2 } from "lucide-react";
import { useState } from "react";

import { useI18n } from "../../app/i18n";
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
import type {
  EnvironmentDto,
  EnvironmentMutationResultDto,
  EnvironmentVariableDto,
  EnvironmentVariableDraftDto,
  RequestWorkspaceSnapshotDto,
} from "../../shared/api/generated/ipc";

type EnvironmentManagerDialogProps = {
  environments: EnvironmentDto[];
  environmentVariables: EnvironmentVariableDto[];
  onCreate: (name: string) => Promise<RequestWorkspaceSnapshotDto>;
  onDelete: (environmentId: string) => Promise<RequestWorkspaceSnapshotDto>;
  onOpenChange: (open: boolean) => void;
  onSave: (
    environmentId: string,
    name: string,
    variables: EnvironmentVariableDraftDto[],
  ) => Promise<EnvironmentMutationResultDto>;
  onSelect: (environmentId: string | null) => Promise<void>;
  open: boolean;
};

export function EnvironmentManagerDialog(props: EnvironmentManagerDialogProps) {
  if (!props.open) {
    return null;
  }
  return <EnvironmentManagerForm {...props} />;
}

function EnvironmentManagerForm({
  environments,
  environmentVariables,
  onCreate,
  onDelete,
  onOpenChange,
  onSave,
  onSelect,
  open,
}: EnvironmentManagerDialogProps) {
  const { formatError, t } = useI18n();
  const selected =
    environments.find((environment) => environment.isSelected) ?? environments[0] ?? null;
  const [managedId, setManagedId] = useState(selected?.id ?? "");
  const [createName, setCreateName] = useState("");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sessionOnly, setSessionOnly] = useState(false);
  const managed = environments.find((environment) => environment.id === managedId) ?? null;

  async function run<T>(operation: () => Promise<T>) {
    setPending(true);
    setError(null);
    try {
      return await operation();
    } catch (caught) {
      setError(formatError(caught));
      return null;
    } finally {
      setPending(false);
    }
  }

  return (
    <Dialog onOpenChange={onOpenChange} open={open}>
      <DialogContent
        aria-describedby="environment-manager-description"
        className="max-h-[90vh] max-w-3xl overflow-y-auto"
      >
        <DialogHeader>
          <DialogTitle>{t("environment.title")}</DialogTitle>
          <DialogDescription id="environment-manager-description">
            {t("environment.description")}
          </DialogDescription>
        </DialogHeader>

        <form
          className="flex gap-2"
          onSubmit={(event) => {
            event.preventDefault();
            const name = createName.trim();
            if (!name) return;
            void run(async () => {
              const snapshot = await onCreate(name);
              const created = snapshot.environments.find((environment) => environment.isSelected);
              setManagedId(created?.id ?? "");
              setCreateName("");
            });
          }}
        >
          <Input
            aria-label={t("environment.newName")}
            disabled={pending}
            onChange={(event) => setCreateName(event.currentTarget.value)}
            placeholder={t("environment.newName")}
            value={createName}
          />
          <Button disabled={pending || !createName.trim()} type="submit">
            {t("environment.create")}
          </Button>
        </form>

        {environments.length > 0 ? (
          <label className="grid gap-1 text-sm font-medium" htmlFor="managed-environment">
            {t("collections.environment")}
            <NativeSelect
              id="managed-environment"
              onChange={(event) => {
                const id = event.currentTarget.value;
                setManagedId(id);
                setSessionOnly(false);
                void run(() => onSelect(id));
              }}
              value={managedId}
            >
              {environments.map((environment) => (
                <option key={environment.id} value={environment.id}>
                  {environment.name}
                </option>
              ))}
            </NativeSelect>
          </label>
        ) : null}

        {managed ? (
          <EnvironmentEditor
            environment={managed}
            key={managed.id}
            onDelete={() => {
              if (!window.confirm(t("environment.deleteConfirm", { name: managed.name }))) {
                return;
              }
              void run(async () => {
                await onDelete(managed.id);
                setManagedId("");
              });
            }}
            onSave={async (name, variables) => {
              const result = await run(async () => {
                const result = await onSave(managed.id, name, variables);
                setSessionOnly(result.secretPersistence === "SESSION_ONLY");
                return result;
              });
              return result !== null;
            }}
            pending={pending}
            variables={environmentVariables.filter(
              (variable) => variable.environmentId === managed.id,
            )}
          />
        ) : null}

        {sessionOnly ? (
          <p className="rounded-md border border-amber-400 bg-amber-50 p-3 text-sm text-amber-950" role="status">
            {t("environment.sessionOnly")}
          </p>
        ) : null}
        {error ? <p className="text-sm text-destructive" role="alert">{error}</p> : null}

        <DialogFooter>
          <Button onClick={() => onOpenChange(false)} type="button" variant="outline">
            {t("common.close")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

type VariableRow = {
  id: number;
  kind: "plain" | "secret";
  name: string;
  previousName: string | null;
  storedSecret: boolean;
  value: string;
};

function EnvironmentEditor({
  environment,
  onDelete,
  onSave,
  pending,
  variables,
}: {
  environment: EnvironmentDto;
  onDelete: () => void;
  onSave: (name: string, variables: EnvironmentVariableDraftDto[]) => Promise<boolean>;
  pending: boolean;
  variables: EnvironmentVariableDto[];
}) {
  const { t } = useI18n();
  const [name, setName] = useState(environment.name);
  const [nextId, setNextId] = useState(variables.length);
  const [rows, setRows] = useState<VariableRow[]>(() =>
    variables.map((variable, index) => ({
      id: index,
      kind: variable.variable.value.type === "PLAIN" ? "plain" : "secret",
      name: variable.variable.name,
      previousName: variable.variable.name,
      storedSecret: variable.variable.value.type === "SECRET_REFERENCE",
      value:
        variable.variable.value.type === "PLAIN" ? variable.variable.value.value : "",
    })),
  );

  function updateRow(id: number, update: Partial<VariableRow>) {
    setRows((current) =>
      current.map((row) => (row.id === id ? { ...row, ...update } : row)),
    );
  }

  function moveRow(index: number, offset: -1 | 1) {
    const target = index + offset;
    if (target < 0 || target >= rows.length) return;
    setRows((current) => {
      const next = [...current];
      [next[index], next[target]] = [next[target], next[index]];
      return next;
    });
  }

  return (
    <form
      className="grid gap-4 rounded-md border border-border p-4"
      onSubmit={(event) => {
        event.preventDefault();
        const trimmedName = name.trim();
        void (async () => {
          const saved = await onSave(
            trimmedName,
            rows.map((row) => ({
            previousName: row.previousName,
            name: row.name.trim(),
            value:
              row.kind === "plain"
                ? { type: "PLAIN", value: row.value }
                : {
                    type: "SECRET",
                    value: row.value.length > 0 ? row.value : row.storedSecret ? null : "",
                  },
            })),
          );
          if (!saved) return;
          setName(trimmedName);
          setRows((current) =>
            current.map((row) => ({
              ...row,
              name: row.name.trim(),
              previousName: row.name.trim(),
              storedSecret: row.kind === "secret",
              value: row.kind === "secret" ? "" : row.value,
            })),
          );
        })();
      }}
    >
      <fieldset className="contents" disabled={pending}>
      <label className="grid gap-1 text-sm font-medium" htmlFor="environment-name">
        {t("environment.name")}
        <Input
          id="environment-name"
          onChange={(event) => setName(event.currentTarget.value)}
          value={name}
        />
      </label>

      <div className="grid gap-2" role="list">
        {rows.map((row, index) => (
          <div
            className="grid gap-2 rounded-md border border-border p-3 sm:grid-cols-[1fr_9rem_1fr_auto]"
            key={row.id}
            role="listitem"
          >
            <Input
              aria-label={`${t("environment.variableName")} ${index + 1}`}
              onChange={(event) => updateRow(row.id, { name: event.currentTarget.value })}
              value={row.name}
            />
            <NativeSelect
              aria-label={`${t("environment.variableType")} ${index + 1}`}
              onChange={(event) =>
                updateRow(row.id, {
                  kind: event.currentTarget.value as "plain" | "secret",
                  value: "",
                })
              }
              value={row.kind}
            >
              <option value="plain">{t("environment.plain")}</option>
              <option value="secret">{t("environment.secret")}</option>
            </NativeSelect>
            <Input
              aria-label={`${t("environment.variableValue")} ${index + 1}`}
              onChange={(event) => updateRow(row.id, { value: event.currentTarget.value })}
              placeholder={
                row.kind === "secret" && row.storedSecret
                  ? t("environment.secretStored")
                  : t("environment.variableValue")
              }
              required={row.kind === "secret" && !row.storedSecret}
              type={row.kind === "secret" ? "password" : "text"}
              value={row.value}
            />
            <div className="flex items-center gap-1">
              <Button
                aria-label={t("environment.moveVariableUp", { index: index + 1 })}
                disabled={index === 0}
                onClick={() => moveRow(index, -1)}
                size="icon"
                type="button"
                variant="ghost"
              >
                <ArrowUp aria-hidden="true" size={15} />
              </Button>
              <Button
                aria-label={t("environment.moveVariableDown", { index: index + 1 })}
                disabled={index === rows.length - 1}
                onClick={() => moveRow(index, 1)}
                size="icon"
                type="button"
                variant="ghost"
              >
                <ArrowDown aria-hidden="true" size={15} />
              </Button>
              <Button
                aria-label={t("environment.removeVariable", { index: index + 1 })}
                onClick={() => setRows((current) => current.filter((item) => item.id !== row.id))}
                size="icon"
                type="button"
                variant="ghost"
              >
                <Trash2 aria-hidden="true" size={15} />
              </Button>
            </div>
          </div>
        ))}
      </div>

      <div className="flex flex-wrap justify-between gap-2">
        <Button
          onClick={() => {
            setRows((current) => [
              ...current,
              {
                id: nextId,
                kind: "plain",
                name: "",
                previousName: null,
                storedSecret: false,
                value: "",
              },
            ]);
            setNextId((current) => current + 1);
          }}
          type="button"
          variant="outline"
        >
          <Plus aria-hidden="true" size={16} />
          {t("environment.addVariable")}
        </Button>
        <div className="flex gap-2">
          <Button disabled={pending} onClick={onDelete} type="button" variant="destructive">
            {t("environment.delete")}
          </Button>
          <Button disabled={pending || !name.trim()} type="submit">
            {t("environment.save")}
          </Button>
        </div>
      </div>
      </fieldset>
    </form>
  );
}
