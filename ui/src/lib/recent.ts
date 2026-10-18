/** The start of "Last 30 days": a unix time, `days` days before `nowSeconds`. */
export const recentSince = (nowSeconds: number, days = 30): number => nowSeconds - days * 86_400;
