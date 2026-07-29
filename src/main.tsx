import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClientProvider } from "@tanstack/react-query";

import { App } from "./app/App";
import { ScreenshotApp } from "./app/ScreenshotApp";
import { queryClient } from "./app/query-client";
import "./app/styles.css";

const rootElement = document.getElementById("root");

if (!rootElement) {
  throw new Error("Postmite root element was not found.");
}

const app = import.meta.env.VITE_POSTMITE_SCREENSHOTS === "1" ? (
  <ScreenshotApp />
) : (
  <QueryClientProvider client={queryClient}>
    <App />
  </QueryClientProvider>
);

createRoot(rootElement).render(<StrictMode>{app}</StrictMode>);
