export function columnsFor(width: number, cell: number, gap: number): number {
  return Math.max(1, Math.floor((width + gap) / (cell + gap)));
}

export function visibleRange(
  total: number,
  cols: number,
  rowH: number,
  scrollTop: number,
  viewH: number,
  overscan = 2,
): { firstIndex: number; endIndex: number; rows: number } {
  if (total <= 0) return { firstIndex: 0, endIndex: 0, rows: 0 };
  const rows = Math.ceil(total / cols);
  const firstRow = Math.max(0, Math.floor(scrollTop / rowH) - overscan);
  const lastRow = Math.min(rows - 1, Math.ceil((scrollTop + viewH) / rowH) + overscan);
  return { firstIndex: firstRow * cols, endIndex: Math.min(total, (lastRow + 1) * cols), rows };
}
