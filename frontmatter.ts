import yaml from 'js-yaml';

export interface FrontMatter {
  title?: string;
  author?: string;
  date?: string;
  tags?: string[];
  theme?: string;
  [key: string]: unknown;
}

export interface ParsedMarkdown {
  frontMatter?: FrontMatter;
  markdown: string;
}

const FRONT_MATTER_REGEX = /^---\s*\n([\s\S]*?)\n---\s*\n?/;

export function parseFrontMatter(markdown: string): ParsedMarkdown {
  const match = markdown.match(FRONT_MATTER_REGEX);

  if (!match) {
    return {markdown};
  }

  const result:ParsedMarkdown = {
    markdown: markdown.slice(match[0].length)
  };
  
  try {
    result.frontMatter = yaml.load(match[1]) as FrontMatter;

    // Normalize tags to array
    if (result.frontMatter.tags && typeof result.frontMatter.tags === 'string') {
      result.frontMatter.tags = [result.frontMatter.tags];
    }

    // Normalize date to string
    if (result.frontMatter.date && typeof result.frontMatter.date === 'object' && 'toISOString' in result.frontMatter.date) {
      result.frontMatter.date = (result.frontMatter.date as Date).toISOString().split('T')[0];
    }

  } catch (error) {
    console.warn('Failed to parse front matter:', error);
  }
  return result;
}

// Get extra metadata fields (excluding title, author, date, tags)
export function getExtraMetadata(frontMatter: FrontMatter): Record<string, unknown> {
  const { title, author, date, tags, theme, ...extra } = frontMatter;
  return extra;
}

// Check if there are extra fields beyond the standard ones
export function hasExtraMetadata(frontMatter: FrontMatter): boolean {
  return Object.keys(getExtraMetadata(frontMatter)).length > 0;
}
