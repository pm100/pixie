/** Calls `fn` once, `ms` after the last call, with the last call's arguments. */
export function debounce<A extends unknown[]>(fn: (...a: A) => void, ms: number) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const wrapped = (...a: A) => {
    clearTimeout(timer);
    timer = setTimeout(() => { timer = undefined; fn(...a); }, ms);
  };
  wrapped.cancel = () => { clearTimeout(timer); timer = undefined; };
  return wrapped;
}
