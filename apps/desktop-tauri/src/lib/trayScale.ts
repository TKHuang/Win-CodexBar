export const TRAY_SCALE_MIN = 100;
export const TRAY_SCALE_MAX = 200;
export const TRAY_SCALE_STEP = 5;

export function clampTrayScalePercent(value: number): number {
  if (!Number.isFinite(value)) return TRAY_SCALE_MIN;
  return Math.min(TRAY_SCALE_MAX, Math.max(TRAY_SCALE_MIN, value));
}

export const TRAY_PANEL_WIDTH_DEFAULT = 310;
export const TRAY_PANEL_WIDTH_MIN = 280;
export const TRAY_PANEL_WIDTH_MAX = 560;

export const OVERVIEW_MAX_COLUMNS = 3;
/** Minimum per-column CSS width before stepping down multi-column Overview. */
export const OVERVIEW_MIN_COLUMN_WIDTH = 260;

export type OverviewColumnCount = 1 | 2 | 3;

export function clampTrayPanelWidth(value: number): number {
  if (!Number.isFinite(value)) return TRAY_PANEL_WIDTH_DEFAULT;
  return Math.round(Math.min(TRAY_PANEL_WIDTH_MAX, Math.max(TRAY_PANEL_WIDTH_MIN, value)));
}

/**
 * Dynamic column count for the Overview card grid so enabling more providers
 * grows horizontally before growing into a tall scrolling strip:
 * - 1–2 cards → 1 column
 * - 3–4 cards → 2 columns (at most 2 rows)
 * - 5+ cards  → 3 columns (at most 2 rows up to the 6-provider overview cap)
 *
 * When `availableCssWidth` is known (work-area width after DPI + tray zoom),
 * the count steps down if each column would shrink below `minColumnWidth`.
 */
export function computeOverviewColumns(
  cardCount: number,
  options?: {
    availableCssWidth?: number | null;
    minColumnWidth?: number;
  },
): OverviewColumnCount {
  if (!Number.isFinite(cardCount) || cardCount <= 2) return 1;
  const desired = Math.min(OVERVIEW_MAX_COLUMNS, Math.ceil(cardCount / 2));
  const minColumnWidth = options?.minColumnWidth ?? OVERVIEW_MIN_COLUMN_WIDTH;
  const available = options?.availableCssWidth;
  if (
    available != null &&
    Number.isFinite(available) &&
    available > 0 &&
    Number.isFinite(minColumnWidth) &&
    minColumnWidth > 0
  ) {
    const maxFitting = Math.max(1, Math.floor(available / minColumnWidth));
    return Math.min(desired, maxFitting) as OverviewColumnCount;
  }
  return desired as OverviewColumnCount;
}

/** Gap kept between the panel and the work-area edges, as for the height. */
const TRAY_WORK_AREA_MARGIN = 16;

/**
 * Caps the scaled panel width (logical px) at the work-area width, which
 * `get_work_area_rect` reports in physical px. A 560 px panel at 200 % scale
 * would otherwise ask for 1,120 px.
 */
export function fitTrayWidthToWorkArea(
  width: number,
  workAreaPhysicalWidth: number | null,
  scaleFactor: number,
): number {
  if (workAreaPhysicalWidth == null || !Number.isFinite(workAreaPhysicalWidth)) return width;
  const scale = Number.isFinite(scaleFactor) && scaleFactor > 0 ? scaleFactor : 1;
  const available = Math.floor(workAreaPhysicalWidth / scale) - TRAY_WORK_AREA_MARGIN;
  if (available <= 0) return width;
  return Math.min(width, available);
}
