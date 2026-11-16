import {describe, it, expect} from 'vitest';
import {getSeasonalThemeSuggestion} from '../seasonalTheme';

// JS Date months are 0-indexed; this helper takes a human month (1-12).
const d = (year: number, month: number, day: number) => new Date(year, month - 1, day);

describe('getSeasonalThemeSuggestion', () => {
  describe('northern hemisphere seasons', () => {
    it('winter in January', () => {
      const s = getSeasonalThemeSuggestion('DE', {date: d(2026, 1, 15)});
      expect(s?.themeName).toBe('winter');
      expect(s?.isHoliday).toBe(false);
      expect(s?.seasonKey).toBe('winter-2026');
    });

    it('spring in May', () => {
      expect(getSeasonalThemeSuggestion('DE', {date: d(2026, 5, 15)})?.themeName).toBe('spring');
    });

    it('summer in July', () => {
      expect(getSeasonalThemeSuggestion('DE', {date: d(2026, 7, 15)})?.themeName).toBe('summer');
    });

    it('autumn in October', () => {
      expect(getSeasonalThemeSuggestion('DE', {date: d(2026, 10, 15)})?.themeName).toBe('autumn');
    });

    it('treats a null region as northern', () => {
      expect(getSeasonalThemeSuggestion(null, {date: d(2026, 7, 15)})?.themeName).toBe('summer');
    });
  });

  describe('southern hemisphere flips the season', () => {
    it('July is winter in Australia', () => {
      expect(getSeasonalThemeSuggestion('AU', {date: d(2026, 7, 15)})?.themeName).toBe('winter');
    });

    it('January is summer in New Zealand', () => {
      expect(getSeasonalThemeSuggestion('NZ', {date: d(2026, 1, 15)})?.themeName).toBe('summer');
    });

    it('is case-insensitive on the region code', () => {
      expect(getSeasonalThemeSuggestion('au', {date: d(2026, 7, 15)})?.themeName).toBe('winter');
    });
  });

  describe('holiday overrides', () => {
    it('suggests Christmas anywhere in December', () => {
      const s = getSeasonalThemeSuggestion('DE', {date: d(2026, 12, 3)});
      expect(s?.themeName).toBe('christmas');
      expect(s?.isHoliday).toBe(true);
      expect(s?.seasonKey).toBe('christmas-2026');
    });

    it('suggests Christmas in December regardless of hemisphere', () => {
      expect(getSeasonalThemeSuggestion('AU', {date: d(2026, 12, 25)})?.themeName).toBe('christmas');
    });

    it('suggests Easter on Easter Sunday (Apr 20, 2025)', () => {
      const s = getSeasonalThemeSuggestion('DE', {date: d(2025, 4, 20)});
      expect(s?.themeName).toBe('easter');
      expect(s?.isHoliday).toBe(true);
    });

    it('suggests Easter within ±7 days (Easter 2026 = Apr 5)', () => {
      expect(getSeasonalThemeSuggestion('DE', {date: d(2026, 3, 29)})?.themeName).toBe('easter'); // 7 days before
      expect(getSeasonalThemeSuggestion('DE', {date: d(2026, 4, 12)})?.themeName).toBe('easter'); // 7 days after
    });

    it('does not suggest Easter more than 7 days away', () => {
      expect(getSeasonalThemeSuggestion('DE', {date: d(2026, 4, 13)})?.themeName).toBe('spring'); // 8 days after
    });
  });

  describe('skipHolidays returns the underlying season', () => {
    it('winter instead of Christmas in December (north)', () => {
      const s = getSeasonalThemeSuggestion('DE', {date: d(2026, 12, 21), skipHolidays: true});
      expect(s?.themeName).toBe('winter');
      expect(s?.isHoliday).toBe(false);
    });

    it('spring instead of Easter', () => {
      expect(getSeasonalThemeSuggestion('DE', {date: d(2025, 4, 20), skipHolidays: true})?.themeName).toBe('spring');
    });

    it('summer in December in the southern hemisphere', () => {
      expect(getSeasonalThemeSuggestion('AU', {date: d(2026, 12, 21), skipHolidays: true})?.themeName).toBe('summer');
    });
  });

  describe('season key year', () => {
    it('bumps December into the next year', () => {
      const s = getSeasonalThemeSuggestion('DE', {date: d(2025, 12, 21), skipHolidays: true});
      expect(s?.seasonKey).toBe('winter-2026');
    });

    it('does not bump non-December months', () => {
      expect(getSeasonalThemeSuggestion('DE', {date: d(2026, 7, 15)})?.seasonKey).toBe('summer-2026');
    });
  });

  it('capitalizes the theme name in the label', () => {
    expect(getSeasonalThemeSuggestion('DE', {date: d(2026, 7, 15)})?.label).toContain('Summer');
  });
});
