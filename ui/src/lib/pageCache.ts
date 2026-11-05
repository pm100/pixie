/**
 * Lazily loads fixed-size pages of a long list. Each reset starts a new
 * generation; responses from an older generation are dropped, so stale rows
 * can never land in a freshly cleared cache.
 */
export class PageCache<T> {
  items: Record<number, T> = {};
  private requested = new Set<number>();
  private generation = 0;

  constructor(
    private fetchPage: (offset: number, limit: number) => Promise<T[]>,
    private pageSize: number,
    private onChange: () => void,
  ) {}

  reset(): void {
    this.generation++;
    this.items = {};
    this.requested.clear();
    this.onChange();
  }

  /** Starts a new generation like `reset`, but leaves the rows on show until the pages are fetched again and replace them (the caller must `ensure` the pages in view).
   *  For a listing that only grew: emptying it would blank every thumbnail at once. */
  refresh(): void {
    this.generation++;
    this.requested.clear();
  }

  /** Make sure every page overlapping [first, endExclusive) is loaded or loading. */
  ensure(first: number, endExclusive: number): void {
    if (endExclusive <= first) return;
    const lastPage = Math.floor((endExclusive - 1) / this.pageSize);
    for (let page = Math.floor(first / this.pageSize); page <= lastPage; page++) {
      if (this.requested.has(page)) continue;
      this.requested.add(page);
      const gen = this.generation;
      this.fetchPage(page * this.pageSize, this.pageSize)
        .then((rows) => {
          if (gen !== this.generation) return;
          rows.forEach((row, i) => (this.items[page * this.pageSize + i] = row));
          this.onChange();
        })
        .catch(() => {
          if (gen === this.generation) this.requested.delete(page);
        });
    }
  }
}
