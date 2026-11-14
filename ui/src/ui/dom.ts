export function element<T extends HTMLElement = HTMLElement>(selector: string): T {
  const node = document.querySelector<T>(selector);
  if (!node) throw new Error("Missing interface element: " + selector);
  return node;
}
export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K, className = "", text?: string,
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}
export function setText(node: Element, text: string): void {
  if (node.textContent !== text) node.textContent = text;
}
export function utc(value: string | Date | null, short = false): string {
  if (!value) return "No measurement time";
  const date = typeof value === "string" ? new Date(value) : value;
  if (!Number.isFinite(date.getTime())) return "No measurement time";
  return new Intl.DateTimeFormat("en-GB", {
    timeZone: "UTC", ...(short ? {} : { day: "2-digit", month: "short", year: "numeric" }),
    hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false,
  }).format(date) + " UTC";
}
export function timestamp(value: string | null): HTMLElement {
  if (!value || !Number.isFinite(Date.parse(value))) return el("span", "", "No measurement time");
  const node = el("time", "", utc(value));
  node.dateTime = value;
  return node;
}
export function numberText(value: number | null): string {
  return value === null || !Number.isFinite(value) ? "--" : value.toLocaleString("en-US", { maximumFractionDigits: 10 });
}
