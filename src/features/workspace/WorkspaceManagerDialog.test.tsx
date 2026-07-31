import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "jest-axe";
import { describe, expect, it, vi } from "vitest";

import { WorkspaceManagerDialog } from "./WorkspaceManagerDialog";

const workspaces = [
  { id: "workspace-1", name: "Personal", isSelected: true, baseDirectory: null },
  { id: "workspace-2", name: "Client", isSelected: false, baseDirectory: null },
];

describe("WorkspaceManagerDialog", () => {
  it("supports keyboard-accessible selection, creation, and rename actions", async () => {
    const user = userEvent.setup();
    const onCreate = vi.fn().mockResolvedValue(undefined);
    const onDelete = vi.fn().mockResolvedValue(undefined);
    const onRename = vi.fn().mockResolvedValue(undefined);
    const onSelect = vi.fn().mockResolvedValue(undefined);
    render(
      <WorkspaceManagerDialog
        onCreate={onCreate}
        onDelete={onDelete}
        onOpenChange={vi.fn()}
        onRename={onRename}
        onSelect={onSelect}
        open
        selectedWorkspaceId="workspace-1"
        workspaces={workspaces}
      />,
    );

    await user.selectOptions(screen.getByLabelText("Workspace"), "workspace-2");
    expect(onSelect).toHaveBeenCalledWith("workspace-2");

    const rename = screen.getByLabelText("Rename selected workspace");
    await user.clear(rename);
    await user.type(rename, "Renamed Client");
    await user.click(screen.getByRole("button", { name: "Rename selected workspace" }));
    expect(onRename).toHaveBeenCalledWith("workspace-2", "Renamed Client");

    await user.type(screen.getByLabelText("New workspace name"), "Local API");
    await user.click(screen.getByRole("button", { name: "Create workspace" }));
    expect(onCreate).toHaveBeenCalledWith("Local API");
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    await user.click(screen.getByRole("button", { name: "Delete selected workspace" }));
    expect(onDelete).toHaveBeenCalledWith("workspace-2");
    confirm.mockRestore();
    expect((await axe(document.body)).violations).toEqual([]);
  });
});
