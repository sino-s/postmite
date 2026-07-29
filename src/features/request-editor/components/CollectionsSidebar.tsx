import { useMemo } from "react";
import { ArrowDown, ArrowUp, Copy, Edit3, FileText, Folder, FolderPlus, Trash2 } from "lucide-react";

import { Button } from "../../../components/ui/button";
import { NativeSelect } from "../../../components/ui/native-select";
import { ScrollArea } from "../../../components/ui/scroll-area";
import type { CollectionFolderDto, EnvironmentDto, SavedRequestDto } from "../../../shared/api/generated/ipc";
import { IconButton } from "./IconButton";
import { useI18n } from "../../../app/i18n";

type CollectionsSidebarProps = {
  environments: EnvironmentDto[];
  folders: CollectionFolderDto[];
  onCreateFolder: (parentCollectionId: string | null) => void;
  onDeleteFolder: (folder: CollectionFolderDto) => void;
  onDeleteRequest: (request: SavedRequestDto) => void;
  onDuplicateFolder: (folder: CollectionFolderDto) => void;
  onDuplicateRequest: (request: SavedRequestDto) => void;
  onMoveFolder: (folder: CollectionFolderDto, direction: -1 | 1) => void;
  onMoveRequest: (request: SavedRequestDto, direction: -1 | 1) => void;
  onOpenRequest: (request: SavedRequestDto) => void;
  onRenameFolder: (folder: CollectionFolderDto) => void;
  onSelectEnvironment: (environmentId: string | null) => void;
  requests: SavedRequestDto[];
};

type TreeRow =
  | {
      kind: "folder";
      id: string;
      depth: number;
      folder: CollectionFolderDto;
    }
  | {
      kind: "request";
      id: string;
      depth: number;
      request: SavedRequestDto;
    };

export function CollectionsSidebar({
  environments,
  folders,
  onCreateFolder,
  onDeleteFolder,
  onDeleteRequest,
  onDuplicateFolder,
  onDuplicateRequest,
  onMoveFolder,
  onMoveRequest,
  onOpenRequest,
  onRenameFolder,
  onSelectEnvironment,
  requests,
}: CollectionsSidebarProps) {
  const { t } = useI18n();
  const rows = useMemo(
    () => buildCollectionRows(folders, requests, null, 0),
    [folders, requests],
  );

  function handleTreeKeyDown(
    event: React.KeyboardEvent<HTMLButtonElement>,
    row: TreeRow,
  ) {
    if (event.key === "Enter" && row.kind === "request") {
      event.preventDefault();
      onOpenRequest(row.request);
      return;
    }
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") {
      return;
    }

    event.preventDefault();
    const items = Array.from(
      event.currentTarget
        .closest("[data-collection-tree]")
        ?.querySelectorAll<HTMLButtonElement>("[data-tree-item]") ?? [],
    );
    const currentIndex = items.indexOf(event.currentTarget);
    const nextIndex =
      event.key === "ArrowDown"
        ? Math.min(items.length - 1, currentIndex + 1)
        : Math.max(0, currentIndex - 1);
    items[nextIndex]?.focus();
  }

  return (
    <aside
      aria-label={t("collections.title")}
      className="flex min-h-0 flex-col border-b border-border bg-muted md:border-b-0 md:border-r"
    >
      <div className="flex h-11 shrink-0 items-center justify-between border-b border-border px-3">
        <h2 className="text-sm font-semibold">{t("collections.title")}</h2>
        <Button
          aria-label={t("collections.newRoot")}
          onClick={() => onCreateFolder(null)}
          size="icon"
          title={t("collections.newRoot")}
          type="button"
          variant="ghost"
        >
          <FolderPlus aria-hidden="true" size={16} />
        </Button>
      </div>
      <div className="border-b border-border px-3 py-3">
        <label className="mb-1 block text-xs font-semibold text-slate-600" htmlFor="environment-select">
          {t("collections.environment")}
        </label>
        <NativeSelect
          id="environment-select"
          onChange={(event) =>
            onSelectEnvironment(event.currentTarget.value || null)
          }
          value={environments.find((environment) => environment.isSelected)?.id ?? ""}
        >
          <option value="">{t("collections.noEnvironment")}</option>
          {environments
            .slice()
            .sort(compareTreeItems)
            .map((environment) => (
              <option key={environment.id} value={environment.id}>
                {environment.name}
              </option>
            ))}
        </NativeSelect>
      </div>
      <ScrollArea
        aria-label={t("collections.tree")}
        className="min-h-32 flex-1 overflow-auto py-2"
        data-collection-tree
        role={rows.length > 0 ? "tree" : undefined}
      >
        {rows.map((row) => {
          const label = row.kind === "folder" ? row.folder.name : row.request.content.name;
          return (
            <div
              className="group flex h-9 items-center gap-1 px-2"
              key={`${row.kind}-${row.id}`}
              role="none"
              style={{ paddingLeft: `${8 + row.depth * 16}px` }}
            >
              <button
                className="inline-flex min-w-0 flex-1 items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-white focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
                data-tree-item
                onClick={() => {
                  if (row.kind === "request") {
                    onOpenRequest(row.request);
                  }
                }}
                onKeyDown={(event) => handleTreeKeyDown(event, row)}
                role="treeitem"
                type="button"
              >
                {row.kind === "folder" ? (
                  <Folder aria-hidden="true" className="shrink-0 text-amber-700" size={16} />
                ) : (
                  <FileText aria-hidden="true" className="shrink-0 text-sky-700" size={16} />
                )}
                <span className="truncate">{label}</span>
              </button>
              {row.kind === "folder" ? (
                <TreeActions
                  onCreate={() => onCreateFolder(row.folder.id)}
                  onDelete={() => onDeleteFolder(row.folder)}
                  onDuplicate={() => onDuplicateFolder(row.folder)}
                  onMoveDown={() => onMoveFolder(row.folder, 1)}
                  onMoveUp={() => onMoveFolder(row.folder, -1)}
                  onRename={() => onRenameFolder(row.folder)}
                />
              ) : (
                <RequestTreeActions
                  onDelete={() => onDeleteRequest(row.request)}
                  onDuplicate={() => onDuplicateRequest(row.request)}
                  onMoveDown={() => onMoveRequest(row.request, 1)}
                  onMoveUp={() => onMoveRequest(row.request, -1)}
                />
              )}
            </div>
          );
        })}
        {rows.length === 0 ? (
          <p className="px-3 py-6 text-sm text-slate-500">{t("collections.noRequests")}</p>
        ) : null}
      </ScrollArea>
    </aside>
  );
}

type TreeActionsProps = {
  onCreate: () => void;
  onDelete: () => void;
  onDuplicate: () => void;
  onMoveDown: () => void;
  onMoveUp: () => void;
  onRename: () => void;
};

function TreeActions({
  onCreate,
  onDelete,
  onDuplicate,
  onMoveDown,
  onMoveUp,
  onRename,
}: TreeActionsProps) {
  const { t } = useI18n();
  return (
    <div className="flex shrink-0 items-center opacity-100 md:opacity-0 md:group-hover:opacity-100 md:focus-within:opacity-100">
      <IconButton label={t("actions.newSubfolder")} onClick={onCreate}>
        <FolderPlus aria-hidden="true" size={14} />
      </IconButton>
      <IconButton label={t("actions.renameFolder")} onClick={onRename}>
        <Edit3 aria-hidden="true" size={14} />
      </IconButton>
      <IconButton label={t("actions.moveFolderUp")} onClick={onMoveUp}>
        <ArrowUp aria-hidden="true" size={14} />
      </IconButton>
      <IconButton label={t("actions.moveFolderDown")} onClick={onMoveDown}>
        <ArrowDown aria-hidden="true" size={14} />
      </IconButton>
      <IconButton label={t("actions.duplicateFolder")} onClick={onDuplicate}>
        <Copy aria-hidden="true" size={14} />
      </IconButton>
      <IconButton label={t("actions.deleteFolder")} onClick={onDelete}>
        <Trash2 aria-hidden="true" size={14} />
      </IconButton>
    </div>
  );
}

type RequestTreeActionsProps = {
  onDelete: () => void;
  onDuplicate: () => void;
  onMoveDown: () => void;
  onMoveUp: () => void;
};

function RequestTreeActions({
  onDelete,
  onDuplicate,
  onMoveDown,
  onMoveUp,
}: RequestTreeActionsProps) {
  const { t } = useI18n();
  return (
    <div className="flex shrink-0 items-center opacity-100 md:opacity-0 md:group-hover:opacity-100 md:focus-within:opacity-100">
      <IconButton label={t("actions.moveRequestUp")} onClick={onMoveUp}>
        <ArrowUp aria-hidden="true" size={14} />
      </IconButton>
      <IconButton label={t("actions.moveRequestDown")} onClick={onMoveDown}>
        <ArrowDown aria-hidden="true" size={14} />
      </IconButton>
      <IconButton label={t("actions.duplicateRequest")} onClick={onDuplicate}>
        <Copy aria-hidden="true" size={14} />
      </IconButton>
      <IconButton label={t("actions.deleteRequest")} onClick={onDelete}>
        <Trash2 aria-hidden="true" size={14} />
      </IconButton>
    </div>
  );
}

function buildCollectionRows(
  folders: CollectionFolderDto[],
  requests: SavedRequestDto[],
  parentCollectionId: string | null,
  depth: number,
): TreeRow[] {
  const childFolders = folders
    .filter((folder) => folder.parentCollectionId === parentCollectionId)
    .sort(compareTreeItems);
  const childRequests = requests
    .filter((request) => request.collectionId === parentCollectionId)
    .sort(compareTreeItems);

  return [
    ...childFolders.flatMap((folder) => [
      { kind: "folder" as const, id: folder.id, depth, folder },
      ...buildCollectionRows(folders, requests, folder.id, depth + 1),
    ]),
    ...childRequests.map((request) => ({
      kind: "request" as const,
      id: request.id,
      depth,
      request,
    })),
  ];
}

function compareTreeItems(
  left: Pick<CollectionFolderDto | SavedRequestDto, "position" | "id">,
  right: Pick<CollectionFolderDto | SavedRequestDto, "position" | "id">,
) {
  return left.position - right.position || left.id.localeCompare(right.id);
}
