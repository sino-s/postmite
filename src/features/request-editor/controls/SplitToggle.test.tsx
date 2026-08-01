import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { SplitToggle } from "./SplitToggle";

describe("SplitToggle", () => {
  it.each([
    {
      expectedIcon: "lucide-square-split-vertical",
      label: "Stack request options above response",
      split: "horizontal" as const,
    },
    {
      expectedIcon: "lucide-square-split-horizontal",
      label: "Place request options beside response",
      split: "vertical" as const,
    },
  ])("matches $split layout with the $label control", ({ expectedIcon, label, split }) => {
    render(<SplitToggle setSplit={vi.fn()} split={split} />);

    const selected = screen.getByRole("button", { name: label });
    const unselected = screen.getAllByRole("button").find((button) => button !== selected);

    expect(selected).toHaveAttribute("aria-pressed", "true");
    expect(selected.querySelector("svg")).toHaveClass(expectedIcon);
    expect(unselected).toHaveAttribute("aria-pressed", "false");
  });

  it("changes the persisted split selection through the button callback", () => {
    const setSplit = vi.fn();
    render(<SplitToggle setSplit={setSplit} split="horizontal" />);

    fireEvent.click(screen.getByRole("button", { name: "Place request options beside response" }));

    expect(setSplit).toHaveBeenCalledWith("vertical");
  });
});
