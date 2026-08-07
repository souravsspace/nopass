import "@nopass/ui/globals.css";

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { Popup } from "../../components/popup";
import { browserBridge } from "../../lib/browser-bridge";

const container = document.querySelector("#root");
if (container) {
  createRoot(container).render(
    <StrictMode>
      <Popup bridge={browserBridge()} />
    </StrictMode>
  );
}
