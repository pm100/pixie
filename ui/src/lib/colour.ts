/** Colour arithmetic for the colour picker: hex strings (#rrggbb) and hue / saturation / value (h 0-360, s and v 0-1). */
export type Hsv = { h: number; s: number; v: number };

/** `#rrggbb` (lower case) from what was typed (`#abc`, `ABC`, `a1b2c3`, with or without the #), or null when it is not a colour. */
export function normaliseHex(text: string): string | null {
  const m = /^#?([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(text.trim());
  if (!m) return null;
  const h = m[1].toLowerCase();
  return '#' + (h.length === 3 ? [...h].map((c) => c + c).join('') : h);
}

export function hexToHsv(hex: string): Hsv {
  const n = normaliseHex(hex) ?? '#000000';
  const r = parseInt(n.slice(1, 3), 16) / 255;
  const g = parseInt(n.slice(3, 5), 16) / 255;
  const b = parseInt(n.slice(5, 7), 16) / 255;
  const max = Math.max(r, g, b);
  const d = max - Math.min(r, g, b);
  let h = 0;
  if (d > 0) {
    if (max === r) h = ((g - b) / d) % 6;
    else if (max === g) h = (b - r) / d + 2;
    else h = (r - g) / d + 4;
    h *= 60;
    if (h < 0) h += 360;
  }
  return { h, s: max === 0 ? 0 : d / max, v: max };
}

export function hsvToHex({ h, s, v }: Hsv): string {
  const f = (n: number) => {
    const k = (n + h / 60) % 6;
    return v - v * s * Math.max(0, Math.min(k, 4 - k, 1));
  };
  const part = (x: number) => Math.round(x * 255).toString(16).padStart(2, '0');
  return '#' + part(f(5)) + part(f(3)) + part(f(1));
}
