import {describe, expect, it} from 'vitest';
import {getMenuIconFrame} from '../menuIconMotion';

describe('Menu FAB icon motion', () => {
  it('moves the dots to separate corners before drawing or filling the triangle', () => {
    const closed = getMenuIconFrame(0);
    const corners = getMenuIconFrame(0.42);

    expect(closed.points.map((point) => point.y)).toEqual([16, 16, 16]);
    expect(corners.points.map((point) => point.y)).toEqual([11, 23, 11]);
    expect(corners.edgeProgress).toBe(0);
    expect(corners.fillProgress).toBe(0);
    expect(getMenuIconFrame(0.75)).toMatchObject({edgeProgress: 1, fillProgress: 0});
    expect(getMenuIconFrame(0.9).fillProgress).toBeGreaterThan(0);
    expect(getMenuIconFrame(1)).toMatchObject({dotOpacity: 0, fillProgress: 1});
  });

  it('draws an open chevron on the other side of the shared closed frame', () => {
    const chevron = getMenuIconFrame(-1);

    expect(chevron).toMatchObject({triangle: false, dotOpacity: 0, edgeProgress: 1, fillProgress: 0});
    expect(chevron.points.map((point) => point.y)).toEqual([13, 20, 13]);
    expect(getMenuIconFrame(-0.000001).points[1].y).toBeCloseTo(getMenuIconFrame(0.000001).points[1].y);
  });
});
