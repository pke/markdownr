import React from 'react';
import {View, StyleSheet} from 'react-native';
import MathJax from 'react-native-mathjax-svg';
import {useMarkdownContext, type MathRendererProps} from 'react-native-nitro-markdown';

// nitro-markdown >= 0.13 renders math with RaTeX, which needs React Native 0.84+.
// Until the app gets there, keep the MathJax SVG rendering of nitro-markdown 0.4.

function normalizeInlineMath(content: string): string {
  return content.replace(/^\$+|\$+$/g, '').trim();
}

export function MathInline({content}: MathRendererProps) {
  const {theme} = useMarkdownContext();
  const latex = normalizeInlineMath(content);
  if (!latex) return null;
  return (
    <View style={styles.inline}>
      <MathJax fontSize={theme.fontSizes.s} color={theme.colors.text} style={styles.transparent}>
        {latex}
      </MathJax>
    </View>
  );
}

export function MathBlock({content}: MathRendererProps) {
  const {theme} = useMarkdownContext();
  if (!content) return null;
  return (
    <View
      style={[
        styles.block,
        {
          marginVertical: theme.spacing.m,
          padding: theme.spacing.l,
          backgroundColor: theme.colors.surface,
          borderRadius: theme.borderRadius.l,
          borderColor: theme.colors.border,
        },
      ]}>
      <MathJax fontSize={theme.fontSizes.l} color={theme.colors.text} style={styles.transparent}>
        {`\\displaystyle ${content}`}
      </MathJax>
    </View>
  );
}

export const mathRenderers = {
  math_inline: (props: MathRendererProps) => <MathInline {...props} />,
  math_block: (props: MathRendererProps) => <MathBlock {...props} />,
};

const styles = StyleSheet.create({
  inline: {
    marginHorizontal: 2,
    justifyContent: 'center',
  },
  block: {
    alignItems: 'center',
    borderWidth: 1,
    // Keep the block from collapsing before MathJax reports its size
    minHeight: 48,
  },
  transparent: {
    backgroundColor: 'transparent',
  },
});
