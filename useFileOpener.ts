import {useCallback, useContext} from 'react';
import * as DocumentPicker from 'expo-document-picker';
import {File} from 'expo-file-system/next';
import {MarkdownContext} from './MarkdownContext';

export function useFileOpener(onOpen?: () => void) {
  const {openFile} = useContext(MarkdownContext);

  return useCallback(async () => {
    onOpen?.();

    try {
      const result = await DocumentPicker.getDocumentAsync({
        type: ['text/markdown', 'text/plain', 'net.daringfireball.markdown', 'public.plain-text'],
        copyToCacheDirectory: true,
      });

      if (!result.canceled && result.assets && result.assets.length > 0) {
        const file = result.assets[0];
        const content = await new File(file.uri).text();
        openFile(content, file.name ?? null);
      }
    } catch (err) {
      console.error('Error picking document:', err);
    }
  }, [openFile, onOpen]);
}
