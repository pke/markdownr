import type {ThemeName} from './themes';
import {customThemes} from './themes';

export type SeasonalSuggestion = {
  themeName: ThemeName;
  label: string;
  seasonKey: string;
  isHoliday: boolean;
};

type Options = {
  date?: Date;
  skipHolidays?: boolean;
};

// Countries predominantly in the Southern Hemisphere
const SOUTHERN_HEMISPHERE = new Set([
  'AU', 'NZ', 'AR', 'CL', 'UY', 'PY', 'BR', 'ZA', 'MZ', 'MW',
  'ZW', 'MG', 'BW', 'NA', 'LS', 'SZ', 'FJ', 'WS', 'TO', 'PG',
]);

function isSouthernHemisphere(regionCode: string | null): boolean {
  if (!regionCode) return false;
  return SOUTHERN_HEMISPHERE.has(regionCode.toUpperCase());
}

/**
 * Compute Easter Sunday for a given year using the Anonymous Gregorian algorithm.
 * Returns a Date object for Easter Sunday.
 */
function getEasterDate(year: number): Date {
  const a = year % 19;
  const b = Math.floor(year / 100);
  const c = year % 100;
  const d = Math.floor(b / 4);
  const e = b % 4;
  const f = Math.floor((b + 8) / 25);
  const g = Math.floor((b - f + 1) / 3);
  const h = (19 * a + b - d - g + 15) % 30;
  const i = Math.floor(c / 4);
  const k = c % 4;
  const l = (32 + 2 * e + 2 * i - h - k) % 7;
  const m = Math.floor((a + 11 * h + 22 * l) / 451);
  const month = Math.floor((h + l - 7 * m + 114) / 31); // 3 = March, 4 = April
  const day = ((h + l - 7 * m + 114) % 31) + 1;
  return new Date(year, month - 1, day);
}

function daysBetween(a: Date, b: Date): number {
  const msPerDay = 86400000;
  const aStart = new Date(a.getFullYear(), a.getMonth(), a.getDate()).getTime();
  const bStart = new Date(b.getFullYear(), b.getMonth(), b.getDate()).getTime();
  return Math.round((bStart - aStart) / msPerDay);
}

type Season = 'winter' | 'spring' | 'summer' | 'autumn';

const NORTHERN_SEASONS: Season[] = [
  'winter',  // Jan
  'winter',  // Feb
  'spring',  // Mar
  'spring',  // Apr
  'spring',  // May
  'summer',  // Jun
  'summer',  // Jul
  'summer',  // Aug
  'autumn',  // Sep
  'autumn',  // Oct
  'autumn',  // Nov
  'winter',  // Dec
];

const SEASON_FLIP: Record<Season, Season> = {
  winter: 'summer',
  summer: 'winter',
  spring: 'autumn',
  autumn: 'spring',
};

function getSeasonForMonth(month: number, southern: boolean): Season {
  const season = NORTHERN_SEASONS[month];
  return southern ? SEASON_FLIP[season] : season;
}

function makeLabel(themeName: ThemeName): string {
  const icon = customThemes[themeName].icon;
  const name = (themeName as string).charAt(0).toUpperCase() + (themeName as string).slice(1);
  return `${icon} ${name}`;
}

/**
 * Compute the year portion of a season key.
 * December's season spans into the next year (e.g. Dec 2025 → winter-2026 in Northern,
 * summer-2026 in Southern), so we bump the year for December.
 */
function seasonYear(date: Date): number {
  return date.getMonth() === 11 ? date.getFullYear() + 1 : date.getFullYear();
}

export function getSeasonalThemeSuggestion(
  regionCode: string | null,
  options?: Options,
): SeasonalSuggestion | null {
  const date = options?.date ?? new Date();
  const skipHolidays = options?.skipHolidays ?? false;
  const month = date.getMonth(); // 0-11
  const day = date.getDate();
  const year = date.getFullYear();
  const southern = isSouthernHemisphere(regionCode);

  // Holiday overrides (checked first, unless skipped)
  if (!skipHolidays) {
    // Christmas: December 1–31
    if (month === 11) {
      return {
        themeName: 'christmas',
        label: makeLabel('christmas'),
        seasonKey: `christmas-${year}`,
        isHoliday: true,
      };
    }

    // Easter: ±7 days from Easter Sunday
    const easter = getEasterDate(year);
    const diff = daysBetween(easter, date);
    if (Math.abs(diff) <= 7) {
      return {
        themeName: 'easter',
        label: makeLabel('easter'),
        seasonKey: `easter-${year}`,
        isHoliday: true,
      };
    }
  }

  // Seasonal suggestion
  const season = getSeasonForMonth(month, southern);
  const sYear = seasonYear(date);

  return {
    themeName: season,
    label: makeLabel(season),
    seasonKey: `${season}-${sYear}`,
    isHoliday: false,
  };
}
