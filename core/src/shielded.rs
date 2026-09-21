//! The wrapper type for protected data (SECURITY.md §4.2, CORE-CRATE.md §4).
//!
//! `Shielded<T>` has no `Debug`, `Display`, `Serialize` or `ToString`, so a log line, a panic
//! message or a JSON reply that tries to carry one does not compile. The plaintext comes out
//! only through [`Shielded::reveal`], which needs a [`Pass`]. Exactly three pass types exist,
//! and this file names all three: [`crate::store::CachePass`] (the sealed cache writer),
//! [`crate::payload::CardPass`] (the card payload) and [`crate::net::SitePass`] (a request
//! to the site the value came from, and the parse of its reply; the owner, 2026-09-21). Each
//! is built only inside its own module. The value is wiped from memory when it drops.
//!
//! A log line cannot print one:
//! ```compile_fail
//! let name = deetshealth_core::Shielded::new(String::from("a name"));
//! println!("{:?}", name);
//! ```
//! Nor can a JSON reply carry one:
//! ```compile_fail
//! fn to_front_end<T: serde::Serialize>(_: &T) {}
//! to_front_end(&deetshealth_core::Shielded::new(String::from("a name")));
//! ```
//! Nor can code outside the core make a pass:
//! ```compile_fail
//! let pass = deetshealth_core::store::CachePass { _only_here: () };
//! ```

use std::cmp::Ordering;
use zeroize::Zeroize;

/// A protected value. Wrap it the moment it is parsed; it stays wrapped until the cache
/// writer seals it or a card payload shows it.
pub struct Shielded<T: Zeroize>(T);

impl<T: Zeroize> Shielded<T> {
    /// Wrap a value just parsed from the site or just unsealed from the cache.
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// The plaintext, for the two holders of a pass only.
    pub fn reveal<P: Pass>(&self, _pass: &P) -> &T {
        &self.0
    }
}

impl<T: Zeroize> Drop for Shielded<T> {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// A copy stays shielded, and is wiped on its own drop.
impl<T: Zeroize + Clone> Clone for Shielded<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

// Comparison, so core can filter and sort in Rust (no protected column is plain in SQLite,
// CORE-CRATE.md §5). A comparison answers "same?" or "which first?", never "what is it?".
impl<T: Zeroize + PartialEq> PartialEq for Shielded<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<T: Zeroize + Eq> Eq for Shielded<T> {}
impl<T: Zeroize + PartialOrd> PartialOrd for Shielded<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
impl<T: Zeroize + Ord> Ord for Shielded<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

mod sealed {
    /// Only this file can implement it, so only this file decides who holds a pass.
    pub trait Sealed {}
}

/// Proof that the caller may read plaintext. Implemented for exactly three types, below.
pub trait Pass: sealed::Sealed {}

impl sealed::Sealed for crate::store::CachePass {}
impl Pass for crate::store::CachePass {}
impl sealed::Sealed for crate::payload::CardPass {}
impl Pass for crate::payload::CardPass {}
impl sealed::Sealed for crate::net::SitePass {}
impl Pass for crate::net::SitePass {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::CachePass;

    #[test]
    fn reveal_needs_a_pass_and_gives_the_value() {
        let v = Shielded::new(String::from("sandbox value"));
        assert_eq!(v.reveal(&CachePass::for_tests()), "sandbox value");
    }

    #[test]
    fn sorts_without_revealing() {
        let mut v = [Shielded::new(3u32), Shielded::new(1), Shielded::new(2)];
        v.sort();
        let pass = CachePass::for_tests();
        let order: Vec<u32> = v.iter().map(|s| *s.reveal(&pass)).collect();
        assert_eq!(order, [1, 2, 3]);
    }

    #[test]
    fn a_clone_is_equal_and_separate() {
        let a = Shielded::new(String::from("x"));
        let b = a.clone();
        drop(a);
        assert_eq!(b.reveal(&CachePass::for_tests()), "x");
    }
}
