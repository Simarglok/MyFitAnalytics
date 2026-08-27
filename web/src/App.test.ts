import { mount, unmount } from "svelte";
import { describe, expect, it, vi } from "vitest";
import App from "./App.svelte";
import { MockTransport } from "./lib/mock-transport";
import type { BootstrapState, ModuleView } from "./lib/types";

const modules: ModuleView[] = [
  {
    id: "bundled-source",
    moduleType: "source",
    version: "1.0.0",
    enabled: true,
    localizationNamespace: "source.bundled",
  },
];

const bootstrap: BootstrapState = {
  productName: "MyFitAnalytics",
  locale: "en-US",
  activeProviders: {},
  modules,
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((complete) => {
    resolve = complete;
  });
  return { promise, resolve };
}

describe("App shell", () => {
  it("renders the product title, locale, loading completion, and module list", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const app = mount(App, {
      target,
      props: { transport: new MockTransport({ bootstrap, modules }) },
    });

    await vi.waitFor(() =>
      expect(target.textContent).toContain("bundled-source"),
    );
    expect(target.textContent).toContain("en-US");
    expect(target.textContent).toContain("bundled-source");
    unmount(app);
  });

  it("shows a loading state until the bootstrap transport resolves", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const pending = deferred<BootstrapState>();
    const app = mount(App, {
      target,
      props: {
        transport: new MockTransport({
          bootstrapPromise: pending.promise,
          modules,
        }),
      },
    });

    expect(target.textContent).toContain("Loading MyFitAnalytics");
    pending.resolve(bootstrap);
    await vi.waitFor(() =>
      expect(target.textContent).toContain("bundled-source"),
    );
    unmount(app);
  });

  it("renders a structured error state from the transport", async () => {
    const target = document.createElement("div");
    document.body.append(target);
    const app = mount(App, {
      target,
      props: {
        transport: new MockTransport({
          error: { code: "module_unavailable", message: "offline" },
        }),
      },
    });

    await vi.waitFor(() =>
      expect(target.textContent).toContain("Unable to load MyFitAnalytics"),
    );
    expect(target.textContent).toContain("module_unavailable");
    expect(target.textContent).toContain("offline");
    unmount(app);
  });
});
