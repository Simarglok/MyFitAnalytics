export function formatDate(
  value: string | null | undefined,
  locale = "en-US",
): string {
  if (!value) return "—";
  const date = new Date(`${value}T00:00:00Z`);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat(locale, {
    dateStyle: "medium",
    timeZone: "UTC",
  }).format(date);
}

export function formatNumber(
  value: number | null | undefined,
  locale = "en-US",
  options: Intl.NumberFormatOptions = {},
): string {
  if (value === null || value === undefined || !Number.isFinite(value))
    return "—";
  return new Intl.NumberFormat(locale, options).format(value);
}

export function formatPercent(
  value: number | null | undefined,
  locale = "en-US",
): string {
  return formatNumber(
    value === null || value === undefined ? value : value / 100,
    locale,
    {
      style: "percent",
      maximumFractionDigits: 0,
    },
  );
}

export function sourceName(name: string): string {
  return name;
}
