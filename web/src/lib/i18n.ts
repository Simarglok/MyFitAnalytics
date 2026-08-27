import { translate } from "./i18n/catalog";

export function message(key: string, fallback?: string): string {
  return translate(key, fallback);
}
