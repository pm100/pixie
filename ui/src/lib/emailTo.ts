/** The same check as the backend's `clean_to` (which has the last word), so a mistake can be shown under the To box before
 *  anything is sent. Addresses are separated by commas, semicolons or spaces. */
const FORBIDDEN = /[^\x21-\x7e]|[<>()[\]\\",:;]/;

/** The first address that does not look like `name@host`, or null when there is none (an empty box is fine). */
export function badAddress(text: string): string | null {
  for (const a of text.split(/[,;\s]+/).filter(Boolean)) {
    const at = a.indexOf('@');
    const host = a.slice(at + 1);
    const ok = a.length <= 254 && !FORBIDDEN.test(a) && at > 0 && !host.includes('@') && host.includes('.') && !host.startsWith('.') && !host.endsWith('.');
    if (!ok) return a;
  }
  return null;
}
