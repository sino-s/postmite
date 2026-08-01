import { fireEvent, render, screen } from "@testing-library/react";
import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
  type ResizableLayout,
  type ResizableLayoutChangedMeta,
} from "./resizable";

afterEach(() => {
  vi.restoreAllMocks();
});

describe("ResizablePanelGroup", () => {
  it("keeps a complete layout through StrictMode ref cleanup", () => {
    const { container } = render(
      <StrictMode>
        <ResizablePanelGroup defaultLayout={{ first: 35, second: 65 }}>
          <ResizablePanel id="first">first</ResizablePanel>
          <ResizableHandle aria-label="Resize strict panels" />
          <ResizablePanel id="second">second</ResizablePanel>
        </ResizablePanelGroup>
      </StrictMode>,
    );

    expect(container.querySelector<HTMLElement>("#first")?.style.flexGrow).toBe("35");
    expect(container.querySelector<HTMLElement>("#second")?.style.flexGrow).toBe("65");
    expect(screen.getByRole("separator", { name: "Resize strict panels" }))
      .toHaveAttribute("aria-valuenow", "35");
  });

  it("coalesces pointer movement to one DOM update per animation frame", () => {
    const frames: FrameRequestCallback[] = [];
    vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
      frames.push(callback);
      return frames.length;
    });
    vi.spyOn(window, "cancelAnimationFrame").mockImplementation(() => undefined);
    const onLayoutChanged = vi.fn<(
      layout: ResizableLayout,
      meta: ResizableLayoutChangedMeta,
    ) => void>();

    const { container } = render(
      <ResizablePanelGroup
        defaultLayout={{ first: 50, second: 50 }}
        onLayoutChanged={onLayoutChanged}
      >
        <ResizablePanel id="first" minSize="20%">first</ResizablePanel>
        <ResizableHandle aria-label="Resize test panels" />
        <ResizablePanel id="second" minSize="20%">second</ResizablePanel>
      </ResizablePanelGroup>,
    );
    const group = container.querySelector<HTMLElement>("[data-group]")!;
    vi.spyOn(group, "getBoundingClientRect").mockReturnValue(
      DOMRect.fromRect({ height: 600, width: 1_000 }),
    );
    const first = container.querySelector<HTMLElement>("#first")!;
    const separator = screen.getByRole("separator", { name: "Resize test panels" });

    fireEvent.pointerDown(separator, { button: 0, clientX: 500, pointerId: 1 });
    fireEvent.pointerMove(separator, { clientX: 560, pointerId: 1 });
    fireEvent.pointerMove(separator, { clientX: 650, pointerId: 1 });

    expect(frames).toHaveLength(1);
    expect(first.style.flexGrow).toBe("50");
    expect(onLayoutChanged).not.toHaveBeenCalled();

    frames[0](16);
    expect(first.style.flexGrow).toBe("65");
    expect(separator).toHaveAttribute("aria-valuenow", "65");
    expect(onLayoutChanged).not.toHaveBeenCalled();

    fireEvent.pointerUp(separator, { clientX: 650, pointerId: 1 });
    expect(onLayoutChanged).toHaveBeenCalledOnce();
    expect(onLayoutChanged).toHaveBeenCalledWith(
      { first: 65, second: 35 },
      { isUserInteraction: true },
    );
  });

  it("supports constrained keyboard resizing without scheduling a frame", () => {
    const onLayoutChanged = vi.fn();
    const { container } = render(
      <ResizablePanelGroup
        defaultLayout={{ first: 40, second: 60 }}
        onLayoutChanged={onLayoutChanged}
      >
        <ResizablePanel id="first" maxSize="70%" minSize="30%">first</ResizablePanel>
        <ResizableHandle aria-label="Resize keyboard panels" />
        <ResizablePanel id="second" minSize="20%">second</ResizablePanel>
      </ResizablePanelGroup>,
    );
    const group = container.querySelector<HTMLElement>("[data-group]")!;
    vi.spyOn(group, "getBoundingClientRect").mockReturnValue(
      DOMRect.fromRect({ height: 600, width: 1_000 }),
    );
    const first = container.querySelector<HTMLElement>("#first")!;
    const separator = screen.getByRole("separator", {
      name: "Resize keyboard panels",
    });

    separator.focus();
    fireEvent.keyDown(separator, { key: "End" });

    expect(separator).toHaveFocus();
    expect(separator).toHaveAttribute("aria-orientation", "vertical");
    expect(separator).toHaveAttribute("aria-controls", "first");
    expect(separator).toHaveAttribute("aria-valuemin", "30");
    expect(separator).toHaveAttribute("aria-valuemax", "70");
    expect(separator).toHaveAttribute("aria-valuenow", "70");
    expect(first.style.flexGrow).toBe("70");
    expect(onLayoutChanged).toHaveBeenLastCalledWith(
      { first: 70, second: 30 },
      { isUserInteraction: true },
    );

    fireEvent.keyDown(separator, { key: "Home" });
    expect(first.style.flexGrow).toBe("30");
  });
});
