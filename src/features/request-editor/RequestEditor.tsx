import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Play,
  Plus,
  RotateCcw,
  Save,
  Trash2,
  X,
} from "lucide-react";
import { useMemo, useState } from "react";

import {
  closeRequestTab,
  openUnsavedRequestTab,
  requestWorkspaceQuery,
  saveRequestDraft,
  updateRequestDraft,
} from "../../shared/api/requests";
import { startRequestExecution } from "../../shared/api/execution";
import {
  workspaceQuery,
  workspaceQueryKey,
} from "../../shared/api/workspaces";
import type {
  OrderedFieldDto,
  RequestContentDto,
  RequestDraftDto,
  RequestTabDto,
} from "../../shared/api/generated/ipc";
import { RawBodyEditor } from "./RawBodyEditor";
import {
  applyQueryToUrl,
  createEmptyField,
  normalizeFieldOrders,
  queryFromUrl,
  sortOrderedFields,
} from "./ordered-fields";

const METHODS = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];

type RequestEditorProps = {
  onExecute?: typeof startRequestExecution;
};

type OverrideMap = Record<string, RequestContentDto>;

export function RequestEditor({
  onExecute = startRequestExecution,
}: RequestEditorProps) {
  const queryClient = useQueryClient();
  const [activeTabId, setActiveTabId] = useState<string | null>(null);
  const [overrides, setOverrides] = useState<OverrideMap>({});
  const [executionStatus, setExecutionStatus] = useState("No request sent");

  const workspaces = useQuery(workspaceQuery);
  const selectedWorkspaceId = workspaces.data?.selectedWorkspaceId;
  const requestWorkspace = useQuery({
    ...requestWorkspaceQuery({ workspaceId: selectedWorkspaceId ?? "" }),
    enabled: Boolean(selectedWorkspaceId),
  });

  const snapshot = requestWorkspace.data;
  const tabs = useMemo(
    () => [...(snapshot?.tabs ?? [])].sort((left, right) => left.position - right.position),
    [snapshot?.tabs],
  );
  const activeTab =
    tabs.find((tab) => tab.id === activeTabId) ??
    tabs.find((tab) => tab.isActive) ??
    tabs[0] ??
    null;
  const activeDraft =
    activeTab && snapshot
      ? snapshot.drafts.find((draft) => draft.id === activeTab.draftId) ?? null
      : null;
  const activeContent =
    activeDraft ? overrides[activeDraft.id] ?? activeDraft.content : null;

  const openTabMutation = useMutation({
    mutationFn: async () => {
      if (!selectedWorkspaceId) {
        throw new Error("No workspace is selected.");
      }
      return openUnsavedRequestTab(queryClient, {
        workspaceId: selectedWorkspaceId,
      });
    },
    onSuccess: (nextSnapshot) => {
      const nextActive =
        nextSnapshot.tabs.find((tab) => tab.isActive) ??
        nextSnapshot.tabs[nextSnapshot.tabs.length - 1];
      setActiveTabId(nextActive?.id ?? null);
    },
  });

  async function persistDraft(draft: RequestDraftDto, content: RequestContentDto) {
    await updateRequestDraft({
      workspaceId: draft.workspaceId,
      draftId: draft.id,
      content,
    });
  }

  function changeActiveDraft(updater: (content: RequestContentDto) => RequestContentDto) {
    if (!activeDraft || !activeContent) {
      return;
    }

    const nextContent = updater(activeContent);
    setOverrides((current) => ({
      ...current,
      [activeDraft.id]: nextContent,
    }));
    void persistDraft(activeDraft, nextContent);
  }

  async function handleSave() {
    if (!activeDraft || !activeContent) {
      return;
    }

    await persistDraft(activeDraft, activeContent);
    const nextSnapshot = await saveRequestDraft(queryClient, {
      workspaceId: activeDraft.workspaceId,
      draftId: activeDraft.id,
    });
    setOverrides((current) => omitKey(current, activeDraft.id));
    setActiveTabId(
      nextSnapshot.tabs.find((tab) => tab.draftId === activeDraft.id)?.id ?? activeTabId,
    );
  }

  async function handleClose(tab: RequestTabDto, decision: "SAVE" | "DISCARD") {
    const draft = snapshot?.drafts.find((item) => item.id === tab.draftId);
    const content = draft ? overrides[draft.id] ?? draft.content : null;
    if (draft && content && decision === "SAVE") {
      await persistDraft(draft, content);
    }

    const nextSnapshot = await closeRequestTab(queryClient, {
      workspaceId: tab.workspaceId,
      tabId: tab.id,
      decision,
    });
    setOverrides((current) => omitKey(current, tab.draftId));
    setActiveTabId(
      nextSnapshot.tabs.find((item) => item.isActive)?.id ??
        nextSnapshot.tabs[0]?.id ??
        null,
    );
  }

  async function handleExecute() {
    if (!activeDraft || !activeContent) {
      return;
    }

    await persistDraft(activeDraft, activeContent);
    const result = await onExecute({
      workspaceId: activeDraft.workspaceId,
      draftId: activeDraft.id,
      content: activeContent,
    });
    setExecutionStatus(`Execution ${result.status}: ${result.executionId}`);
  }

  function handleEditorKeyDown(event: React.KeyboardEvent<HTMLElement>) {
    if (!(event.ctrlKey || event.metaKey)) {
      return;
    }

    if (event.key === "s") {
      event.preventDefault();
      void handleSave();
    }
    if (event.key === "Enter") {
      event.preventDefault();
      void handleExecute();
    }
  }

  if (workspaces.isPending || requestWorkspace.isPending) {
    return (
      <main className="flex min-h-screen items-center justify-center bg-slate-100 text-slate-950">
        <p className="text-sm">Loading Postmite</p>
      </main>
    );
  }

  if (workspaces.isError || requestWorkspace.isError || !selectedWorkspaceId) {
    return (
      <main className="flex min-h-screen items-center justify-center bg-slate-100 p-6 text-slate-950">
        <section
          aria-labelledby="request-editor-error"
          className="w-full max-w-xl rounded-md border border-red-300 bg-white p-5"
        >
          <h1 id="request-editor-error" className="text-base font-semibold">
            Request workspace unavailable
          </h1>
          <button
            className="mt-4 inline-flex items-center gap-2 rounded-md border border-slate-300 bg-white px-3 py-2 text-sm font-medium hover:bg-slate-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
            onClick={() => {
              void queryClient.invalidateQueries({ queryKey: workspaceQueryKey });
            }}
            type="button"
          >
            <RotateCcw aria-hidden="true" size={16} />
            Retry
          </button>
        </section>
      </main>
    );
  }

  return (
    <main
      className="flex min-h-screen flex-col bg-slate-100 text-slate-950"
      onKeyDown={handleEditorKeyDown}
    >
      <header className="flex min-h-12 items-center justify-between border-b border-slate-300 bg-white px-4">
        <h1 className="text-sm font-semibold">Postmite</h1>
        <button
          className="inline-flex h-8 items-center gap-2 rounded-md bg-slate-900 px-3 text-sm font-medium text-white hover:bg-slate-700 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500 disabled:cursor-not-allowed disabled:opacity-60"
          disabled={openTabMutation.isPending}
          onClick={() => openTabMutation.mutate()}
          type="button"
        >
          <Plus aria-hidden="true" size={16} />
          New
        </button>
      </header>

      <div className="flex min-h-0 flex-1 flex-col">
        <TabStrip
          activeTabId={activeTab?.id ?? null}
          drafts={snapshot?.drafts ?? []}
          onActivate={setActiveTabId}
          onClose={(tab) => void handleClose(tab, isDraftDirty(tab.draftId, snapshot?.drafts ?? [], overrides) ? "SAVE" : "DISCARD")}
          tabs={tabs}
        />

        {activeDraft && activeContent ? (
          <section
            aria-label="Request editor"
            className="grid min-h-0 flex-1 grid-rows-[auto_minmax(0,1fr)_auto] gap-4 p-4"
          >
            <RequestLine
              content={activeContent}
              onChange={changeActiveDraft}
              onExecute={() => void handleExecute()}
              onSave={() => void handleSave()}
              saving={false}
            />
            <div className="grid min-h-0 gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(360px,0.9fr)]">
              <section className="flex min-h-0 flex-col gap-4">
                <FieldTable
                  fields={activeContent.query}
                  legend="Params"
                  onChange={(fields) =>
                    changeActiveDraft((content) => ({
                      ...content,
                      query: fields,
                      url: applyQueryToUrl(content.url, fields),
                    }))
                  }
                />
                <FieldTable
                  fields={activeContent.headers}
                  legend="Headers"
                  onChange={(fields) =>
                    changeActiveDraft((content) => ({
                      ...content,
                      headers: fields,
                    }))
                  }
                />
              </section>
              <RawBodyEditor
                onChange={(body) =>
                  changeActiveDraft((content) => ({
                    ...content,
                    body,
                  }))
                }
                value={activeContent.body}
              />
            </div>
            <footer
              aria-live="polite"
              className="min-h-10 rounded-md border border-slate-300 bg-white px-3 py-2 text-sm text-slate-700"
            >
              {executionStatus}
            </footer>
          </section>
        ) : (
          <section className="flex flex-1 items-center justify-center p-6">
            <button
              className="inline-flex h-10 items-center gap-2 rounded-md bg-slate-900 px-4 text-sm font-medium text-white hover:bg-slate-700 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
              onClick={() => openTabMutation.mutate()}
              type="button"
            >
              <Plus aria-hidden="true" size={16} />
              New Request
            </button>
          </section>
        )}
      </div>
    </main>
  );
}

type TabStripProps = {
  activeTabId: string | null;
  drafts: RequestDraftDto[];
  onActivate: (tabId: string) => void;
  onClose: (tab: RequestTabDto) => void;
  tabs: RequestTabDto[];
};

function TabStrip({
  activeTabId,
  drafts,
  onActivate,
  onClose,
  tabs,
}: TabStripProps) {
  return (
    <nav
      aria-label="Request tabs"
      className="flex min-h-11 items-stretch overflow-x-auto border-b border-slate-300 bg-white"
    >
      {tabs.map((tab) => {
        const dirty = isDraftDirty(tab.draftId, drafts, {});
        return (
          <div
            className="flex min-w-44 items-center border-r border-slate-300"
            key={tab.id}
          >
            <button
              aria-current={activeTabId === tab.id ? "page" : undefined}
              className="min-w-0 flex-1 px-3 py-2 text-left text-sm hover:bg-slate-100 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500 aria-[current=page]:bg-sky-50"
              onClick={() => onActivate(tab.id)}
              type="button"
            >
              <span className="block truncate">
                {tab.title}
                {dirty ? " *" : ""}
              </span>
            </button>
            <button
              aria-label={`Close ${tab.title}`}
              className="mr-1 inline-flex h-8 w-8 items-center justify-center rounded-md text-slate-600 hover:bg-slate-100 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
              onClick={() => onClose(tab)}
              type="button"
            >
              <X aria-hidden="true" size={16} />
            </button>
          </div>
        );
      })}
    </nav>
  );
}

type RequestLineProps = {
  content: RequestContentDto;
  onChange: (updater: (content: RequestContentDto) => RequestContentDto) => void;
  onExecute: () => void;
  onSave: () => void;
  saving: boolean;
};

function RequestLine({
  content,
  onChange,
  onExecute,
  onSave,
  saving,
}: RequestLineProps) {
  return (
    <div className="grid gap-3 rounded-md border border-slate-300 bg-white p-3 md:grid-cols-[180px_140px_minmax(0,1fr)_auto_auto]">
      <label className="sr-only" htmlFor="request-name">
        Name
      </label>
      <input
        className="h-10 min-w-0 rounded-md border border-slate-300 px-3 text-sm focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
        id="request-name"
        onChange={(event) =>
          onChange((current) => ({
            ...current,
            name: event.currentTarget.value,
          }))
        }
        placeholder="Request name"
        value={content.name}
      />
      <label className="sr-only" htmlFor="request-method">
        Method
      </label>
      <select
        className="h-10 rounded-md border border-slate-300 bg-white px-3 text-sm font-medium focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
        id="request-method"
        onChange={(event) =>
          onChange((current) => ({
            ...current,
            method: event.currentTarget.value,
          }))
        }
        value={content.method}
      >
        {METHODS.map((method) => (
          <option key={method} value={method}>
            {method}
          </option>
        ))}
      </select>
      <label className="sr-only" htmlFor="request-url">
        URL
      </label>
      <input
        className="h-10 min-w-0 rounded-md border border-slate-300 px-3 text-sm focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
        id="request-url"
        onChange={(event) => {
          const url = event.currentTarget.value;
          onChange((current) => ({
            ...current,
            url,
            query: queryFromUrl(url),
          }));
        }}
        placeholder="https://example.test/resource?tag=one&tag="
        value={content.url}
      />
      <button
        className="inline-flex h-10 items-center justify-center gap-2 rounded-md border border-slate-300 bg-white px-3 text-sm font-medium hover:bg-slate-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
        disabled={saving}
        onClick={onSave}
        type="button"
      >
        <Save aria-hidden="true" size={16} />
        Save
      </button>
      <button
        className="inline-flex h-10 items-center justify-center gap-2 rounded-md bg-sky-700 px-3 text-sm font-semibold text-white hover:bg-sky-600 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
        onClick={onExecute}
        type="button"
      >
        <Play aria-hidden="true" size={16} />
        Send
      </button>
    </div>
  );
}

type FieldTableProps = {
  fields: OrderedFieldDto[];
  legend: "Params" | "Headers";
  onChange: (fields: OrderedFieldDto[]) => void;
};

function FieldTable({ fields, legend, onChange }: FieldTableProps) {
  const orderedFields = sortOrderedFields(fields);

  function updateField(
    index: number,
    updater: (field: OrderedFieldDto) => OrderedFieldDto,
  ) {
    const nextFields = orderedFields.map((field, fieldIndex) =>
      fieldIndex === index ? updater(field) : field,
    );
    onChange(normalizeFieldOrders(nextFields));
  }

  return (
    <fieldset className="min-h-0 rounded-md border border-slate-300 bg-white p-3">
      <div className="mb-3 flex items-center justify-between gap-3">
        <legend className="text-sm font-semibold text-slate-950">{legend}</legend>
        <button
          className="inline-flex h-8 items-center gap-2 rounded-md border border-slate-300 px-2 text-xs font-medium hover:bg-slate-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
          onClick={() =>
            onChange([
              ...orderedFields,
              createEmptyField(orderedFields.length),
            ])
          }
          type="button"
        >
          <Plus aria-hidden="true" size={14} />
          Add
        </button>
      </div>
      <div className="overflow-x-auto">
        <table className="w-full min-w-[560px] table-fixed border-collapse text-sm">
          <thead>
            <tr className="border-y border-slate-200 bg-slate-50 text-left text-xs font-semibold uppercase text-slate-600">
              <th className="w-14 px-2 py-2">On</th>
              <th className="px-2 py-2">Name</th>
              <th className="px-2 py-2">Value</th>
              <th className="w-12 px-2 py-2">
                <span className="sr-only">Actions</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {orderedFields.map((field, index) => (
              <tr className="border-b border-slate-200" key={field.order}>
                <td className="px-2 py-2">
                  <input
                    aria-label={`${legend} row ${index + 1} enabled`}
                    checked={field.enabled}
                    className="h-4 w-4 rounded border-slate-300 text-sky-700 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
                    onChange={(event) =>
                      updateField(index, (current) => ({
                        ...current,
                        enabled: event.currentTarget.checked,
                      }))
                    }
                    type="checkbox"
                  />
                </td>
                <td className="px-2 py-2">
                  <input
                    aria-label={`${legend} row ${index + 1} name`}
                    className="h-9 w-full rounded-md border border-slate-300 px-2 focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
                    onChange={(event) =>
                      updateField(index, (current) => ({
                        ...current,
                        name: event.currentTarget.value,
                      }))
                    }
                    value={field.name}
                  />
                </td>
                <td className="px-2 py-2">
                  <input
                    aria-label={`${legend} row ${index + 1} value`}
                    className="h-9 w-full rounded-md border border-slate-300 px-2 focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
                    onChange={(event) =>
                      updateField(index, (current) => ({
                        ...current,
                        value: event.currentTarget.value,
                      }))
                    }
                    value={field.value}
                  />
                </td>
                <td className="px-2 py-2">
                  <button
                    aria-label={`Remove ${legend} row ${index + 1}`}
                    className="inline-flex h-8 w-8 items-center justify-center rounded-md text-slate-600 hover:bg-slate-100 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
                    onClick={() =>
                      onChange(
                        normalizeFieldOrders(
                          orderedFields.filter((_, fieldIndex) => fieldIndex !== index),
                        ),
                      )
                    }
                    type="button"
                  >
                    <Trash2 aria-hidden="true" size={15} />
                  </button>
                </td>
              </tr>
            ))}
            {orderedFields.length === 0 ? (
              <tr>
                <td className="px-2 py-5 text-center text-sm text-slate-500" colSpan={4}>
                  No {legend.toLowerCase()}
                </td>
              </tr>
            ) : null}
          </tbody>
        </table>
      </div>
    </fieldset>
  );
}

function isDraftDirty(
  draftId: string,
  drafts: RequestDraftDto[],
  overrides: OverrideMap,
) {
  const draft = drafts.find((item) => item.id === draftId);
  return Boolean(draft?.isDirty || overrides[draftId]);
}

function omitKey<T>(record: Record<string, T>, key: string) {
  const next = { ...record };
  delete next[key];
  return next;
}
