export interface ModuleView {
  id: string;
  moduleType: 'source' | 'dashboard' | 'locale';
  version: string;
  enabled: boolean;
  localizationNamespace: string;
}

export interface BootstrapState {
  productName: string;
  locale: string;
  activeProviders: Record<string, string>;
  modules: ModuleView[];
}
