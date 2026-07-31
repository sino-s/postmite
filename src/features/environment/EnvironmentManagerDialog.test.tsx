import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "jest-axe";
import { describe, expect, it, vi } from "vitest";

import type { RequestWorkspaceSnapshotDto } from "../../shared/api/generated/ipc";
import { EnvironmentManagerDialog } from "./EnvironmentManagerDialog";

const snapshot: RequestWorkspaceSnapshotDto = {
  workspaceId: "workspace-1",
  collectionFolders: [],
  environments: [
    {
      id: "environment-1",
      workspaceId: "workspace-1",
      name: "Development",
      position: 0,
      isSelected: true,
    },
  ],
  collectionVariables: [],
  environmentVariables: [
    {
      environmentId: "environment-1",
      workspaceId: "workspace-1",
      variable: { name: "baseUrl", value: { type: "PLAIN", value: "http://localhost" } },
    },
    {
      environmentId: "environment-1",
      workspaceId: "workspace-1",
      variable: {
        name: "token",
        value: { type: "SECRET_REFERENCE", reference: "secret://postmite/reference" },
      },
    },
  ],
  savedRequests: [],
  drafts: [],
  tabs: [],
};

describe("EnvironmentManagerDialog", () => {
  it("submits ordered plain and Secret variables without revealing stored Secrets", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn().mockResolvedValue({
      snapshot,
      secretPersistence: "SESSION_ONLY",
    });
    const onCreate = vi.fn().mockResolvedValue(snapshot);
    const onDelete = vi.fn().mockResolvedValue(snapshot);
    render(
      <EnvironmentManagerDialog
        environments={snapshot.environments}
        environmentVariables={snapshot.environmentVariables}
        onCreate={onCreate}
        onDelete={onDelete}
        onOpenChange={vi.fn()}
        onSave={onSave}
        onSelect={vi.fn().mockResolvedValue(undefined)}
        open
      />,
    );

    expect(screen.getByLabelText("Variable value 2")).toHaveAttribute("type", "password");
    expect(screen.getByLabelText("Variable value 2")).toHaveValue("");
    await user.clear(screen.getByLabelText("Variable value 1"));
    await user.type(screen.getByLabelText("Variable value 1"), "http://127.0.0.1:18080");
    await user.click(screen.getByRole("button", { name: "Move variable 2 up" }));
    await user.click(screen.getByRole("button", { name: "Save environment" }));

    expect(onSave).toHaveBeenCalledWith("environment-1", "Development", [
      {
        previousName: "token",
        name: "token",
        value: { type: "SECRET", value: null },
      },
      {
        previousName: "baseUrl",
        name: "baseUrl",
        value: { type: "PLAIN", value: "http://127.0.0.1:18080" },
      },
    ]);
    expect(screen.getByLabelText("Variable value 1")).toHaveValue("");
    expect(await screen.findByRole("status")).toHaveTextContent("session only");
    await user.type(screen.getByLabelText("New environment name"), "Staging");
    await user.click(screen.getByRole("button", { name: "Create environment" }));
    expect(onCreate).toHaveBeenCalledWith("Staging");
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    await user.click(screen.getByRole("button", { name: "Delete environment" }));
    expect(onDelete).toHaveBeenCalledWith("environment-1");
    confirm.mockRestore();
    expect((await axe(document.body)).violations).toEqual([]);
  });

  it("locks editor controls while a Secret save is pending", async () => {
    const user = userEvent.setup();
    let finishSave: ((value: { snapshot: RequestWorkspaceSnapshotDto; secretPersistence: null }) => void) | undefined;
    const onSave = vi.fn().mockImplementation(
      () =>
        new Promise((resolve) => {
          finishSave = resolve;
        }),
    );
    render(
      <EnvironmentManagerDialog
        environments={snapshot.environments}
        environmentVariables={snapshot.environmentVariables}
        onCreate={vi.fn().mockResolvedValue(snapshot)}
        onDelete={vi.fn().mockResolvedValue(snapshot)}
        onOpenChange={vi.fn()}
        onSave={onSave}
        onSelect={vi.fn().mockResolvedValue(undefined)}
        open
      />,
    );

    await user.click(screen.getByRole("button", { name: "Save environment" }));

    expect(screen.getByLabelText("Variable name 1")).toBeDisabled();
    expect(screen.getByRole("button", { name: "Add variable" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Delete environment" })).toBeDisabled();

    finishSave?.({ snapshot, secretPersistence: null });
    expect(await screen.findByLabelText("Variable name 1")).toBeEnabled();
  });
});
