import { GripVertical } from "lucide-react";
import {
  createContext,
  useCallback,
  useContext,
  useId,
  useRef,
  type CSSProperties,
  type HTMLAttributes,
  type KeyboardEvent,
  type PointerEvent,
  type ReactNode,
} from "react";

import { cn } from "../../shared/lib/utils";

export type ResizableLayout = Record<string, number>;

export type ResizableLayoutChangedMeta = {
  isUserInteraction: boolean;
};

type Orientation = "horizontal" | "vertical";
type Size = number | string;

type PanelRegistration = {
  defaultSize?: Size;
  element: HTMLDivElement;
  id: string;
  maxSize?: Size;
  minSize?: Size;
};

type DragState = {
  firstId: string;
  firstStartSize: number;
  groupSize: number;
  maximum: number;
  minimum: number;
  pointerStart: number;
  secondId: string;
};

type ControllerOptions = {
  defaultLayout?: ResizableLayout;
  onLayoutChanged?: (
    layout: ResizableLayout,
    meta: ResizableLayoutChangedMeta,
  ) => void;
  orientation: Orientation;
};

class SplitController {
  private defaultLayout?: ResizableLayout;
  private drag: DragState | null = null;
  private frame: number | null = null;
  private groupElement: HTMLDivElement | null = null;
  private layout: ResizableLayout | null = null;
  private latestPointer = 0;
  private onLayoutChanged?: ControllerOptions["onLayoutChanged"];
  private orientation: Orientation = "horizontal";
  private panels = new Map<string, PanelRegistration>();
  private separatorElement: HTMLDivElement | null = null;

  configure({ defaultLayout, onLayoutChanged, orientation }: ControllerOptions) {
    this.defaultLayout = defaultLayout;
    this.onLayoutChanged = onLayoutChanged;
    this.orientation = orientation;
  }

  setGroupElement = (element: HTMLDivElement | null) => {
    this.groupElement = element;
    if (!element) {
      this.cancelFrame();
      this.drag = null;
      return;
    }
    this.initializeLayout();
    this.syncLayoutConstraintsFromDom();
  };

  setSeparatorElement = (element: HTMLDivElement | null) => {
    this.separatorElement = element;
    this.updateSeparatorValue();
  };

  registerPanel = (registration: PanelRegistration | null, id: string) => {
    if (registration) {
      this.panels.set(id, registration);
    } else {
      this.panels.delete(id);
      this.layout = null;
    }
    this.initializeLayout();
  };

  onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || !this.groupElement) return;

    const panels = this.orderedPanels();
    if (panels.length !== 2) return;

    this.initializeLayout();
    if (!this.layout) return;

    const groupRect = this.groupElement.getBoundingClientRect();
    const groupSize = this.orientation === "horizontal"
      ? groupRect.width
      : groupRect.height;
    if (groupSize <= 0) return;

    const [first, second] = panels;
    this.syncLayoutFromDom(first, second, groupSize);
    const firstMinimum = sizeToPercent(first.minSize, groupSize, 0);
    const firstMaximum = sizeToPercent(first.maxSize, groupSize, 100);
    const secondMinimum = sizeToPercent(second.minSize, groupSize, 0);
    const secondMaximum = sizeToPercent(second.maxSize, groupSize, 100);

    this.drag = {
      firstId: first.id,
      firstStartSize: this.layout[first.id],
      groupSize,
      maximum: Math.min(firstMaximum, 100 - secondMinimum),
      minimum: Math.max(firstMinimum, 100 - secondMaximum),
      pointerStart: this.pointerPosition(event),
      secondId: second.id,
    };
    this.updateSeparatorRange(this.drag.minimum, this.drag.maximum);
    this.latestPointer = this.drag.pointerStart;
    this.groupElement.dataset.resizing = "true";
    event.currentTarget.setPointerCapture(event.pointerId);
    event.preventDefault();
  };

  onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
    if (!this.drag) return;

    this.latestPointer = this.pointerPosition(event);
    if (this.frame === null) {
      this.frame = window.requestAnimationFrame(() => {
        this.frame = null;
        this.applyPendingPointer();
      });
    }
    event.preventDefault();
  };

  onPointerEnd = (event: PointerEvent<HTMLDivElement>) => {
    if (!this.drag) return;

    this.latestPointer = this.pointerPosition(event);
    this.cancelFrame();
    this.applyPendingPointer();
    this.drag = null;
    delete this.groupElement?.dataset.resizing;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    this.notifyCompletedLayout();
    event.preventDefault();
  };

  onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (!this.groupElement) return;

    const panels = this.orderedPanels();
    if (panels.length !== 2) return;
    this.initializeLayout();
    if (!this.layout) return;

    const groupRect = this.groupElement.getBoundingClientRect();
    const groupSize = this.orientation === "horizontal"
      ? groupRect.width
      : groupRect.height;
    if (groupSize <= 0) return;

    const [first, second] = panels;
    this.syncLayoutFromDom(first, second, groupSize);
    const minimum = Math.max(
      sizeToPercent(first.minSize, groupSize, 0),
      100 - sizeToPercent(second.maxSize, groupSize, 100),
    );
    const maximum = Math.min(
      sizeToPercent(first.maxSize, groupSize, 100),
      100 - sizeToPercent(second.minSize, groupSize, 0),
    );
    const step = event.shiftKey ? 10 : 1;
    this.updateSeparatorRange(minimum, maximum);
    let next: number | null = null;

    if (event.key === "Home") next = minimum;
    if (event.key === "End") next = maximum;
    if (
      (this.orientation === "horizontal" && event.key === "ArrowLeft")
      || (this.orientation === "vertical" && event.key === "ArrowUp")
    ) {
      next = this.layout[first.id] - step;
    }
    if (
      (this.orientation === "horizontal" && event.key === "ArrowRight")
      || (this.orientation === "vertical" && event.key === "ArrowDown")
    ) {
      next = this.layout[first.id] + step;
    }
    if (next === null) return;

    this.applyLayout(first.id, second.id, clamp(next, minimum, maximum));
    this.notifyCompletedLayout();
    event.preventDefault();
  };

  private applyLayout(firstId: string, secondId: string, firstSize: number) {
    if (!this.layout) return;

    this.layout[firstId] = roundLayout(firstSize);
    this.layout[secondId] = roundLayout(100 - firstSize);
    for (const panel of this.panels.values()) {
      const size = this.layout[panel.id];
      if (size === undefined) continue;
      panel.element.style.flexGrow = String(size);
    }
    this.updateSeparatorValue();
  }

  private applyPendingPointer() {
    if (!this.drag) return;

    const deltaPixels = this.latestPointer - this.drag.pointerStart;
    const nextSize = this.drag.firstStartSize + (deltaPixels / this.drag.groupSize) * 100;
    this.applyLayout(
      this.drag.firstId,
      this.drag.secondId,
      clamp(nextSize, this.drag.minimum, this.drag.maximum),
    );
  }

  private cancelFrame() {
    if (this.frame !== null) {
      window.cancelAnimationFrame(this.frame);
      this.frame = null;
    }
  }

  private initializeLayout() {
    const panels = this.orderedPanels();
    if (panels.length !== 2 || this.layout) return;

    const [first, second] = panels;
    const firstDefault = layoutDefault(
      this.defaultLayout?.[first.id],
      first.defaultSize,
    );
    const secondDefault = layoutDefault(
      this.defaultLayout?.[second.id],
      second.defaultSize,
    );
    const firstSize = firstDefault
      ?? (secondDefault === undefined ? 50 : 100 - secondDefault);
    const secondSize = secondDefault ?? 100 - firstSize;
    const total = firstSize + secondSize || 100;

    this.layout = {
      [first.id]: roundLayout((firstSize / total) * 100),
      [second.id]: roundLayout((secondSize / total) * 100),
    };
    this.applyLayout(first.id, second.id, this.layout[first.id]);
    this.syncLayoutConstraintsFromDom();
  }

  panelConstraintStyle(minSize?: Size, maxSize?: Size): CSSProperties {
    const minimum = sizeToCss(minSize);
    const maximum = sizeToCss(maxSize);
    return this.orientation === "horizontal"
      ? { maxWidth: maximum, minWidth: minimum ?? 0 }
      : { maxHeight: maximum, minHeight: minimum ?? 0 };
  }

  private syncLayoutFromDom(
    first: PanelRegistration,
    second: PanelRegistration,
    groupSize: number,
  ) {
    if (!this.layout) return;
    const firstRect = first.element.getBoundingClientRect();
    const firstPixels = this.orientation === "horizontal"
      ? firstRect.width
      : firstRect.height;
    if (firstPixels <= 0) return;
    const firstSize = roundLayout((firstPixels / groupSize) * 100);
    this.layout[first.id] = firstSize;
    this.layout[second.id] = roundLayout(100 - firstSize);
    this.updateSeparatorValue();
  }

  private syncLayoutConstraintsFromDom() {
    if (!this.groupElement || !this.layout) return;
    const panels = this.orderedPanels();
    if (panels.length !== 2) return;
    const groupRect = this.groupElement.getBoundingClientRect();
    const groupSize = this.orientation === "horizontal"
      ? groupRect.width
      : groupRect.height;
    if (groupSize <= 0) return;
    const [first, second] = panels;
    this.syncLayoutFromDom(first, second, groupSize);
    const minimum = Math.max(
      sizeToPercent(first.minSize, groupSize, 0),
      100 - sizeToPercent(second.maxSize, groupSize, 100),
    );
    const maximum = Math.min(
      sizeToPercent(first.maxSize, groupSize, 100),
      100 - sizeToPercent(second.minSize, groupSize, 0),
    );
    this.updateSeparatorRange(minimum, maximum);
  }

  private notifyCompletedLayout() {
    if (!this.layout) return;
    this.onLayoutChanged?.({ ...this.layout }, { isUserInteraction: true });
  }

  private orderedPanels() {
    return [...this.panels.values()].sort((left, right) => {
      if (left.element === right.element) return 0;
      return left.element.compareDocumentPosition(right.element)
        & Node.DOCUMENT_POSITION_FOLLOWING
        ? -1
        : 1;
    });
  }

  private pointerPosition(event: PointerEvent<HTMLDivElement>) {
    return this.orientation === "horizontal" ? event.clientX : event.clientY;
  }

  private updateSeparatorValue() {
    if (!this.separatorElement || !this.layout) return;
    const first = this.orderedPanels()[0];
    if (!first) return;
    this.separatorElement.setAttribute("aria-controls", first.id);
    this.separatorElement.setAttribute(
      "aria-valuenow",
      String(Math.round(this.layout[first.id])),
    );
  }

  private updateSeparatorRange(minimum: number, maximum: number) {
    if (!this.separatorElement) return;
    this.separatorElement.setAttribute("aria-valuemin", String(Math.ceil(minimum)));
    this.separatorElement.setAttribute("aria-valuemax", String(Math.floor(maximum)));
  }
}

const SplitControllerContext = createContext<SplitController | null>(null);

type ResizablePanelGroupProps = HTMLAttributes<HTMLDivElement> & {
  defaultLayout?: ResizableLayout;
  onLayoutChanged?: ControllerOptions["onLayoutChanged"];
  orientation?: Orientation;
};

const ResizablePanelGroup = ({
  children,
  className,
  defaultLayout,
  onLayoutChanged,
  orientation = "horizontal",
  style,
  ...props
}: ResizablePanelGroupProps) => {
  const controllerRef = useRef<SplitController | null>(null);
  if (!controllerRef.current) {
    controllerRef.current = new SplitController();
  }
  const controller = controllerRef.current;
  controller.configure({ defaultLayout, onLayoutChanged, orientation });

  return (
    <SplitControllerContext.Provider value={controller}>
      <div
        {...props}
        className={cn(
          "flex h-full w-full overflow-hidden",
          orientation === "vertical" && "flex-col",
          className,
        )}
        data-group="true"
        ref={controller.setGroupElement}
        style={style}
      >
        {children}
      </div>
    </SplitControllerContext.Provider>
  );
};

type ResizablePanelProps = Omit<HTMLAttributes<HTMLDivElement>, "id"> & {
  children?: ReactNode;
  defaultSize?: Size;
  id?: string;
  maxSize?: Size;
  minSize?: Size;
};

const ResizablePanel = ({
  children,
  className,
  defaultSize,
  id,
  maxSize,
  minSize,
  style,
  ...props
}: ResizablePanelProps) => {
  const controller = useRequiredController();
  const generatedId = useId();
  const panelId = id ?? generatedId;
  const registerPanel = useCallback((element: HTMLDivElement | null) => {
    controller.registerPanel(
      element
        ? { defaultSize, element, id: panelId, maxSize, minSize }
        : null,
      panelId,
    );
  }, [controller, defaultSize, maxSize, minSize, panelId]);

  return (
    <div
      {...props}
      data-panel="true"
      id={panelId}
      ref={registerPanel}
      style={{
        display: "flex",
        flexBasis: 0,
        flexGrow: layoutDefault(undefined, defaultSize) ?? 1,
        flexShrink: 1,
        minHeight: 0,
        minWidth: 0,
        overflow: "hidden",
        ...controller.panelConstraintStyle(minSize, maxSize),
      }}
    >
      <div
        className={className}
        style={{
          flexGrow: 1,
          maxHeight: "100%",
          maxWidth: "100%",
          minHeight: 0,
          minWidth: 0,
          overflow: "auto",
          ...style,
        }}
      >
        {children}
      </div>
    </div>
  );
};

const ResizableHandle = ({
  className,
  orientation = "horizontal",
  withHandle,
  onKeyDown,
  onPointerCancel,
  onPointerDown,
  onPointerMove,
  onPointerUp,
  ...props
}: HTMLAttributes<HTMLDivElement> & {
  orientation?: Orientation;
  withHandle?: boolean;
}) => {
  const controller = useRequiredController();
  return (
    <div
      {...props}
      aria-orientation={orientation === "horizontal" ? "vertical" : "horizontal"}
      aria-valuemax={100}
      aria-valuemin={0}
      className={cn(
        "relative flex shrink-0 touch-none select-none items-center justify-center bg-border focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring",
        orientation === "horizontal"
          ? "w-px cursor-col-resize after:absolute after:inset-y-0 after:left-1/2 after:w-1 after:-translate-x-1/2"
          : "h-px w-full cursor-row-resize after:absolute after:left-0 after:h-1 after:w-full after:translate-x-0 after:-translate-y-1/2",
        className,
      )}
      onKeyDown={(event) => {
        controller.onKeyDown(event);
        onKeyDown?.(event);
      }}
      onPointerCancel={(event) => {
        controller.onPointerEnd(event);
        onPointerCancel?.(event);
      }}
      onPointerDown={(event) => {
        controller.onPointerDown(event);
        onPointerDown?.(event);
      }}
      onPointerMove={(event) => {
        controller.onPointerMove(event);
        onPointerMove?.(event);
      }}
      onPointerUp={(event) => {
        controller.onPointerEnd(event);
        onPointerUp?.(event);
      }}
      ref={controller.setSeparatorElement}
      role="separator"
      tabIndex={0}
    >
      {withHandle ? (
        <div className="z-10 flex h-4 w-3 items-center justify-center rounded-sm border border-border bg-background">
          <GripVertical aria-hidden="true" className="h-3 w-3" />
        </div>
      ) : null}
    </div>
  );
};

function clamp(value: number, minimum: number, maximum: number) {
  return Math.min(maximum, Math.max(minimum, value));
}

function layoutDefault(layoutSize: number | undefined, defaultSize: Size | undefined) {
  if (layoutSize !== undefined && Number.isFinite(layoutSize)) return layoutSize;
  if (defaultSize === undefined) return undefined;
  const parsed = typeof defaultSize === "number" ? defaultSize : Number.parseFloat(defaultSize);
  return Number.isFinite(parsed) ? parsed : undefined;
}

function roundLayout(value: number) {
  return Math.round(value * 10_000) / 10_000;
}

function sizeToPercent(size: Size | undefined, groupSize: number, fallback: number) {
  if (size === undefined) return fallback;
  if (typeof size === "number") return (size / groupSize) * 100;

  const parsed = Number.parseFloat(size);
  if (!Number.isFinite(parsed)) return fallback;
  if (size.endsWith("px")) return (parsed / groupSize) * 100;
  return parsed;
}

function sizeToCss(size: Size | undefined) {
  if (size === undefined) return undefined;
  if (typeof size === "number") return `${size}px`;
  const parsed = Number.parseFloat(size);
  if (!Number.isFinite(parsed)) return undefined;
  if (size.endsWith("px") || size.endsWith("%")) return size;
  return `${parsed}%`;
}

function useRequiredController() {
  const controller = useContext(SplitControllerContext);
  if (!controller) {
    throw new Error("Resizable components must be rendered inside ResizablePanelGroup");
  }
  return controller;
}

export { ResizableHandle, ResizablePanel, ResizablePanelGroup };
