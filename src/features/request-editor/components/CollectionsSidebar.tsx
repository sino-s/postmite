import { useMemo } from "react";
import { ArrowDown, ArrowUp, Copy, Edit3, FileText, Folder, FolderPlus, Trash2 } from "lucide-react";

import type { CollectionFolderDto, EnvironmentDto, SavedRequestDto } from "../../../shared/api/generated/ipc";
import { IconButton } from "./IconButton";

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
      aria-label="Collections"
      className="flex min-h-0 flex-col border-b border-slate-300 bg-slate-50 md:border-b-0 md:border-r"
    >
      <div className="flex h-11 shrink-0 items-center justify-between border-b border-slate-300 px-3">
        <h2 className="text-sm font-semibold">Collections</h2>
        <button
          aria-label="New root folder"
          className="inline-flex h-8 w-8 items-center justify-center rounded-md text-slate-700 hover:bg-white focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
          onClick={() => onCreateFolder(null)}
          title="New root folder"
          type="button"
        >
          <FolderPlus aria-hidden="true" size={16} />
        </button>
      </div>
      <div className="border-b border-slate-300 px-3 py-3">
        <label className="mb-1 block text-xs font-semibold text-slate-600" htmlFor="environment-select">
          Environment
        </label>
        <select
          className="h-9 w-full rounded-md border border-slate-300 bg-white px-2 text-sm focus:border-sky-500 focus:outline focus:outline-2 focus:outline-sky-500"
          id="environment-select"
          onChange={(event) =>
            onSelectEnvironment(event.currentTarget.value || null)
          }
          value={environments.find((environment) => environment.isSelected)?.id ?? ""}
        >
          <option value="">No environment</option>
          {environments
            .slice()
            .sort(compareTreeItems)
            .map((environment) => (
              <option key={environment.id} value={environment.id}>
                {environment.name}
              </option>
            ))}
        </select>
      </div>
      <div
        aria-label="Collection tree"
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
          <p className="px-3 py-6 text-sm text-slate-500">No saved requests</p>
        ) : null}
      </div>
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
  return (
    <div className="flex shrink-0 items-center opacity-100 md:opacity-0 md:group-hover:opacity-100 md:focus-within:opacity-100">
      <IconButton label="New subfolder" onClick={onCreate}>
        <FolderPlus aria-hidden="true" size={14} />
      </IconButton>
      <IconButton label="Rename folder" onClick={onRename}>
        <Edit3 aria-hidden="true" size={14} />
      </IconButton>
      <IconButton label="Move folder up" onClick={onMoveUp}>
        <ArrowUp aria-hidden="true" size={14} />
      </IconButton>
      <IconButton label="Move folder down" onClick={onMoveDown}>
        <ArrowDown aria-hidden="true" size={14} />
      </IconButton>
      <IconButton label="Duplicate folder" onClick={onDuplicate}>
        <Copy aria-hidden="true" size={14} />
      </IconButton>
      <IconButton label="Delete folder" onClick={onDelete}>
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
  return (
    <div className="flex shrink-0 items-center opacity-100 md:opacity-0 md:group-hover:opacity-100 md:focus-within:opacity-100">
      <IconButton label="Move request up" onClick={onMoveUp}>
        <ArrowUp aria-hidden="true" size={14} />
      </IconButton>
      <IconButton label="Move request down" onClick={onMoveDown}>
        <ArrowDown aria-hidden="true" size={14} />
      </IconButton>
      <IconButton label="Duplicate request" onClick={onDuplicate}>
        <Copy aria-hidden="true" size={14} />
      </IconButton>
      <IconButton label="Delete request" onClick={onDelete}>
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
