import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Trash2 } from "lucide-react";
import { describe, expect, it, vi } from "vitest";

import { IconButton } from "./IconButton";

describe("IconButton", () => {
  it("keeps icon-only commands named and exposes a tooltip", async () => {
    const user = userEvent.setup();
    const onClick = vi.fn();

    render(
      <IconButton label="Delete request" onClick={onClick}>
        <Trash2 aria-hidden="true" size={16} />
      </IconButton>,
    );

    const button = screen.getByRole("button", { name: "Delete request" });
    await user.hover(button);

    expect(await screen.findByRole("tooltip")).toHaveTextContent("Delete request");

    await user.click(button);
    expect(onClick).toHaveBeenCalledTimes(1);
  });
});
