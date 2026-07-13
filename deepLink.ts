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

/**
 * Open a file that arrived via a deep link: parse the URL, read the file, and
 * hand the content to `openFile`. Returns true if a file was opened, false if
 * the URL wasn't a usable file link. Read errors propagate to the caller.
 *
 * `readFileText` and `openFile` are injected so this is testable without the
 * native file system or React state.
 */
export async function openDeepLink(
  url: string | null | undefined,
  readFileText: (uri: string) => Promise<string>,
  openFile: (content: string, name: string | null, fileUri: string) => void,
  platform: typeof Platform.OS = Platform.OS,
): Promise<boolean> {
  const parsed = parseDeepLinkFileUri(url, platform);
  if (!parsed) return false;
  const content = await readFileText(parsed.fileUri);
  openFile(content, parsed.name, parsed.fileUri);
  return true;
}
