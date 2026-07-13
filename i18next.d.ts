import 'i18next';
import type {en} from './i18n';

// Type the t() function against the English resource so keys are checked at
// compile time and autocompleted. `en` is the source of truth.
declare module 'i18next' {
  interface CustomTypeOptions {
    defaultNS: 'translation';
    resources: {translation: typeof en};
  }
}
