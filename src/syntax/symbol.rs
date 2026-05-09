use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// A symbol is a lightweight handle — a `u32` index into the context's symbol table.
///
/// Symbols are created via [`Context::mk_symbol`](super::Context::mk_symbol) and are
/// only meaningful within the context that created them.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Symbol(pub(crate) u32);

impl Symbol {
    /// Return the raw integer index for this symbol.
    pub fn as_u32(self) -> u32 {
        self.0
    }

    /// Construct a symbol from a raw index. This is the inverse of [`as_u32`](Self::as_u32).
    ///
    /// # Safety (logical)
    /// The caller must ensure the index refers to a valid symbol in the intended context.
    pub fn from_raw(index: u32) -> Self {
        Self(index)
    }
}

impl fmt::Debug for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Symbol({})", self.0)
    }
}

/// An ordered set of symbols.
pub type SymbolSet = BTreeSet<Symbol>;

/// An ordered map from symbols to values.
pub type SymbolMap<V> = BTreeMap<Symbol, V>;
