import { normalizeTransportError } from './transport';
import type { AppTransport } from './transport';
import type { BootstrapState, ModuleView } from './types';

export interface MockTransportOptions {
  bootstrap?: BootstrapState;
  bootstrapPromise?: Promise<BootstrapState>;
  modules?: ModuleView[];
  error?: unknown;
}

export class MockTransport implements AppTransport {
  private readonly options: MockTransportOptions;

  constructor(options: MockTransportOptions = {}) {
    this.options = options;
  }

  async getBootstrapState(): Promise<BootstrapState> {
    if (this.options.error !== undefined) throw normalizeTransportError(this.options.error);
    if (this.options.bootstrapPromise) return this.options.bootstrapPromise;
    if (this.options.bootstrap) return this.options.bootstrap;
    return {
      productName: 'MyFitAnalytics',
      locale: 'en-US',
      activeProviders: {},
      modules: this.options.modules ?? [],
    };
  }

  async listModules(): Promise<ModuleView[]> {
    if (this.options.error !== undefined) throw normalizeTransportError(this.options.error);
    return this.options.modules ?? this.options.bootstrap?.modules ?? [];
  }
}
