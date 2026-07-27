import { render, screen } from "@testing-library/react";

import { App } from "./App";

describe("App", () => {
  it("renders the empty Postmite shell", () => {
    render(<App />);

    expect(
      screen.getByRole("heading", { level: 1, name: "Postmite" }),
    ).toBeInTheDocument();
  });
});
