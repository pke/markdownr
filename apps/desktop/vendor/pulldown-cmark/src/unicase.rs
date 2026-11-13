//! Markdownr: minimal stand-in for the `unicase` crate. Link labels compare by
//! Unicode lowercase using std's own tables instead of a second set of
//! case-folding tables (saves ~20 KB in the final binary).

use core::hash::{Hash, Hasher};
use core::ops::Deref;

#[derive(Clone, Debug, Default)]
pub struct UniCase<S>(S);

impl<S: AsRef<str>> UniCase<S> {
    pub fn new(s: S) -> Self {
        UniCase(s)
    }
    fn folded(&self) -> impl Iterator<Item = char> + '_ {
        // Full case folding differs from lowercasing mainly for ß/ẞ → "ss".
        self.0
            .as_ref()
            .chars()
            .flat_map(char::to_lowercase)
            .flat_map(|c| if c == '\u{DF}' { ['s', 's'].into_iter().take(2) } else { [c, c].into_iter().take(1) })
    }
}

impl<S> Deref for UniCase<S> {
    type Target = S;
    fn deref(&self) -> &S {
        &self.0
    }
}

impl<S: AsRef<str>> PartialEq for UniCase<S> {
    fn eq(&self, other: &Self) -> bool {
        self.folded().eq(other.folded())
    }
}

impl<S: AsRef<str>> Eq for UniCase<S> {}

impl<S: AsRef<str>> Hash for UniCase<S> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for c in self.folded() {
            state.write_u32(c as u32);
        }
        state.write_u8(0xff);
    }
}
