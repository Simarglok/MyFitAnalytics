const FALLBACK_MESSAGES: Record<string, string> = {
  'app.title': 'MyFitAnalytics',
  'app.loading': 'Loading MyFitAnalytics',
  'app.error': 'Unable to load MyFitAnalytics',
  'modules.title': 'Installed modules',
};

export function message(key: string, fallback = FALLBACK_MESSAGES[key] ?? key): string {
  return FALLBACK_MESSAGES[key] ?? fallback;
}
