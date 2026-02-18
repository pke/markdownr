import {useCallback, useContext} from 'react';
import * as DocumentPicker from 'expo-document-picker';
import {File} from 'expo-file-system/next';
import {MarkdownContext} from './MarkdownContext';

export function useFileOpener(onOpen?: () => void) {
  const {setMarkdownContent, setFileName} = useContext(MarkdownContext);

  return useCallback(async () => {
    onOpen?.();

    try {
      const result = await DocumentPicker.getDocumentAsync({
        type: ['text/markdown', 'text/plain'],
        copyToCacheDirectory: true,
      });

      if (!result.canceled && result.assets && result.assets.length > 0) {
        const file = result.assets[0];
        const fileExtension = file.name?.toLowerCase().split('.').pop();

        if (fileExtension === 'md' || fileExtension === 'markdown') {
          const content = await new File(file.uri).text();
          setMarkdownContent(content);
          setFileName(file.name ?? null);
        } else {
          console.warn('Please select a .md or .markdown file');
        }
      }
    } catch (err) {
      console.error('Error picking document:', err);
    }
  }, [setMarkdownContent, setFileName, onOpen]);
}
