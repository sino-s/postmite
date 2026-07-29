import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Button } from "./button";

describe("Button", () => {
  it("applies variant, size, and disabled states through the local primitive", () => {
    render(
      <Button disabled size="sm" variant="destructive">
        Delete
      </Button>,
    );

    const button = screen.getByRole("button", { name: "Delete" });
    expect(button).toBeDisabled();
    expect(button).toHaveClass("bg-destructive");
    expect(button).toHaveClass("h-8");
  });
});
