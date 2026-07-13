import {Platform} from 'react-native';

export type ParsedDeepLink = {
  /** URI to read the file content from. */
  fileUri: string;
  /** Display name (last path segment). */
  name: string;
};

/**
 * Parse a file deep-link URL into the URI to read and a display name.
 *
 * iOS delivers percent-encoded `file://` URLs (from Files.app / "Open in…"),
 * so we strip the scheme, decode, and rebuild it. Android delivers `content://`
 * or `file://` URIs that expo-file-system reads as-is.
 *
 * Returns null for empty input or malformed percent-encoding (the caller then
 * ignores the link rather than crashing).
 */
export function parseDeepLinkFileUri(
  url: string | null | undefined,
  platform: typeof Platform.OS = Platform.OS,
): ParsedDeepLink | null {
  if (!url) return null;

  try {
    let filePath = url;
    if (platform === 'ios') {
      filePath = decodeURIComponent(url.replace('file://', ''));
    }
    const fileUri = platform === 'android' ? url : `file://${filePath}`;
    const name = url.split('/').pop() || 'Unknown';
    return {fileUri, name};
  } catch {
    // Malformed percent-encoding (e.g. a lone '%') — ignore the link.
    return null;
  }
}
