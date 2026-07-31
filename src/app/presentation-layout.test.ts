import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  defaultRequestResponseLayout,
  loadRequestResponseLayout,
  saveRequestResponseLayout,
} from "./presentation-layout";

describe("request/response presentation layout", () => {
  beforeEach(() => window.localStorage.clear());
  afterEach(() => vi.restoreAllMocks());

  it("uses the current defaults when storage is empty or malformed", () => {
    const storage = window.localStorage;

    expect(loadRequestResponseLayout(storage, "horizontal")).toEqual({
      request: 52,
      response: 48,
    });
    expect(loadRequestResponseLayout(storage, "vertical")).toEqual({
      request: 56,
      response: 44,
    });

    storage.setItem("postmite.requestResponseLayout.horizontal", "not-json");
    storage.setItem(
      "postmite.requestResponseLayout.vertical",
      JSON.stringify({ request: 95, response: 5 }),
    );

    expect(loadRequestResponseLayout(storage, "horizontal")).toEqual(
      defaultRequestResponseLayout("horizontal"),
    );
    expect(loadRequestResponseLayout(storage, "vertical")).toEqual(
      defaultRequestResponseLayout("vertical"),
    );
  });

  it("saves completed user layouts independently by orientation", () => {
    saveRequestResponseLayout(
      window.localStorage,
      "horizontal",
      { request: 61, response: 39 },
      { isUserInteraction: true },
    );
    saveRequestResponseLayout(
      window.localStorage,
      "vertical",
      { request: 43, response: 57 },
      { isUserInteraction: true },
    );

    expect(loadRequestResponseLayout(window.localStorage, "horizontal")).toEqual({
      request: 61,
      response: 39,
    });
    expect(loadRequestResponseLayout(window.localStorage, "vertical")).toEqual({
      request: 43,
      response: 57,
    });
  });

  it("ignores non-user changes and invalid layouts", () => {
    const setItem = vi.spyOn(Storage.prototype, "setItem");

    saveRequestResponseLayout(
      window.localStorage,
      "horizontal",
      { request: 60, response: 40 },
      { isUserInteraction: false },
    );
    saveRequestResponseLayout(
      window.localStorage,
      "horizontal",
      { request: Number.NaN, response: Number.NaN },
      { isUserInteraction: true },
    );

    expect(setItem).not.toHaveBeenCalled();
  });
});
