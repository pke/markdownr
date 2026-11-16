// Minimal mock for react-native Platform.select used by themes
export const Platform = {
  OS: 'ios',
  select: (options: Record<string, unknown>) => options.ios ?? options.default,
};
