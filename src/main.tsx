import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { TooltipProvider } from "./components/ui/tooltip";
import "./index.css";

const root = document.getElementById("root");

if (!root) {
  throw new Error("React root element was not found.");
}

createRoot(root).render(
  <StrictMode>
    <ErrorBoundary>
      <TooltipProvider delayDuration={200}>
        <App />
      </TooltipProvider>
    </ErrorBoundary>
  </StrictMode>
);
