import { mount } from "svelte";
import App from "./App.svelte";
import { MockTransport, type MockTransportOptions } from "./lib/mock-transport";
import { TauriTransport } from "./lib/tauri-transport";
import "./styles.css";

interface BrowserMockConfig {
  dashboardAvailability?: MockTransportOptions["dashboardAvailability"];
  health?: MockTransportOptions["health"];
}

declare global {
  interface Window {
    __MFA_MOCK__?: boolean;
    __MFA_MOCK_CONFIG__?: BrowserMockConfig;
  }
}

const target = document.getElementById("app");
if (!target) throw new Error("App mount target is missing");

const mockConfig = window.__MFA_MOCK_CONFIG__ ?? {};
const transport = window.__MFA_MOCK__
  ? new MockTransport({
      dashboardAvailability: mockConfig.dashboardAvailability,
      health: mockConfig.health,
      qualityItems: [
        {
          id: "mock-quality-1",
          code: null,
          itemType: "import",
          severity: "warning",
          message: "One deterministic quality issue",
          status: "failed",
          assetId: "mock-asset-1",
        },
      ],
    })
  : new TauriTransport();

mount(App, {
  target,
  props: { transport },
});
