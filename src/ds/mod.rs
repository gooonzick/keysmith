//! Keysmith's design system: tokens first, then the components built on them. Depends only on
//! `rune` and `rune-kit`, never on the rest of the app.
#![allow(dead_code, unused_imports)]

mod tokens;
#[cfg(test)]
mod tests;

pub use tokens::*;
