use std::marker::PhantomData;

use crate::num::QQ;

use super::symbol::Symbol;
use super::types::{TypArith, TypFo};

// Note: typing of nodes is done by `Context::id_typ`, which has access to the
// symbol table needed to resolve the result type of `App` nodes.

// ---------------------------------------------------------------------------
// Internal representation
// ---------------------------------------------------------------------------

/// Raw interned expression ID (index into the context arena).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct ExprId(pub(crate) u32);

/// Internal s-expression node stored in the arena.
///
/// All child references are `ExprId`s — this is the hash-consed representation.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) enum SExprNode {
    // --- Arithmetic ---
    Real(QQ),
    Add(Vec<ExprId>),
    Mul(Vec<ExprId>),
    Div(ExprId, ExprId),
    Mod(ExprId, ExprId),
    Floor(ExprId),
    Neg(ExprId),

    // --- Variables & applications ---
    Var(u32, TypFo),
    App(Symbol, Vec<ExprId>),

    // --- Array ---
    Select(ExprId, ExprId),
    Store(ExprId, ExprId, ExprId),

    // --- Boolean ---
    True,
    False,
    And(Vec<ExprId>),
    Or(Vec<ExprId>),
    Not(ExprId),
    Eq(ExprId, ExprId),
    Leq(ExprId, ExprId),
    Lt(ExprId, ExprId),
    ArrEq(ExprId, ExprId),
    Exists(String, TypFo, ExprId),
    Forall(String, TypFo, ExprId),

    // --- Conditional ---
    Ite(ExprId, ExprId, ExprId),
}

// ---------------------------------------------------------------------------
// Public typed handles
// ---------------------------------------------------------------------------

/// An arithmetic term (integer or real sort).
///
/// `C` is a phantom type marker — no bounds on `C` are required for any trait impls.
pub struct ArithTerm<C> {
    pub(crate) id: ExprId,
    pub(crate) _phantom: PhantomData<C>,
}

/// An array term.
pub struct ArrTerm<C> {
    pub(crate) id: ExprId,
    pub(crate) _phantom: PhantomData<C>,
}

/// A boolean formula.
pub struct Formula<C> {
    pub(crate) id: ExprId,
    pub(crate) _phantom: PhantomData<C>,
}

/// A generic term (either arithmetic or array).
pub enum Term<C> {
    Arith(ArithTerm<C>),
    Arr(ArrTerm<C>),
}

/// Any expression (term or formula).
pub enum Expr<C> {
    ArithTerm(ArithTerm<C>),
    ArrTerm(ArrTerm<C>),
    Formula(Formula<C>),
}

// --- Manual trait impls to avoid requiring C: Clone/Copy/etc. ---

macro_rules! impl_handle_traits {
    ($T:ident) => {
        impl<C> Clone for $T<C> {
            fn clone(&self) -> Self {
                *self
            }
        }
        impl<C> Copy for $T<C> {}
        impl<C> PartialEq for $T<C> {
            fn eq(&self, other: &Self) -> bool {
                self.id == other.id
            }
        }
        impl<C> Eq for $T<C> {}
        impl<C> std::hash::Hash for $T<C> {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.id.hash(state);
            }
        }
    };
}

impl_handle_traits!(ArithTerm);
impl_handle_traits!(ArrTerm);
impl_handle_traits!(Formula);

macro_rules! impl_enum_traits {
    ($T:ident { $($Variant:ident($Inner:ident)),+ }) => {
        impl<C> Clone for $T<C> {
            fn clone(&self) -> Self {
                *self
            }
        }
        impl<C> Copy for $T<C> {}
        impl<C> PartialEq for $T<C> {
            fn eq(&self, other: &Self) -> bool {
                match (self, other) {
                    $( (Self::$Variant(a), Self::$Variant(b)) => a == b, )+
                    _ => false,
                }
            }
        }
        impl<C> Eq for $T<C> {}
        impl<C> std::hash::Hash for $T<C> {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                std::mem::discriminant(self).hash(state);
                match self {
                    $( Self::$Variant(inner) => inner.hash(state), )+
                }
            }
        }
        impl<C> std::fmt::Debug for $T<C> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $( Self::$Variant(inner) => f.debug_tuple(stringify!($Variant)).field(inner).finish(), )+
                }
            }
        }
    };
}

impl_enum_traits!(Term { Arith(ArithTerm), Arr(ArrTerm) });
impl_enum_traits!(Expr { ArithTerm(ArithTerm), ArrTerm(ArrTerm), Formula(Formula) });

impl<C> From<ArithTerm<C>> for Expr<C> {
    fn from(t: ArithTerm<C>) -> Self {
        Self::ArithTerm(t)
    }
}

impl<C> From<ArrTerm<C>> for Expr<C> {
    fn from(t: ArrTerm<C>) -> Self {
        Self::ArrTerm(t)
    }
}

impl<C> From<Formula<C>> for Expr<C> {
    fn from(f: Formula<C>) -> Self {
        Self::Formula(f)
    }
}

impl<C> From<Term<C>> for Expr<C> {
    fn from(t: Term<C>) -> Self {
        match t {
            Term::Arith(a) => Self::ArithTerm(a),
            Term::Arr(a) => Self::ArrTerm(a),
        }
    }
}

impl<C> From<ArithTerm<C>> for Term<C> {
    fn from(t: ArithTerm<C>) -> Self {
        Self::Arith(t)
    }
}

impl<C> From<ArrTerm<C>> for Term<C> {
    fn from(t: ArrTerm<C>) -> Self {
        Self::Arr(t)
    }
}

// --- Debug impls ---

impl<C> std::fmt::Debug for ArithTerm<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ArithTerm(#{})", self.id.0)
    }
}

impl<C> std::fmt::Debug for ArrTerm<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ArrTerm(#{})", self.id.0)
    }
}

impl<C> std::fmt::Debug for Formula<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Formula(#{})", self.id.0)
    }
}

// --- Expr utility methods ---

impl<C> Expr<C> {
    /// Get the raw `ExprId` for this expression.
    pub(crate) fn id(self) -> ExprId {
        match self {
            Self::ArithTerm(t) => t.id,
            Self::ArrTerm(t) => t.id,
            Self::Formula(f) => f.id,
        }
    }
}

impl<C> Term<C> {
    pub(crate) fn id(self) -> ExprId {
        match self {
            Self::Arith(t) => t.id,
            Self::Arr(t) => t.id,
        }
    }
}

// ---------------------------------------------------------------------------
// View types — destructured representations for pattern matching
// ---------------------------------------------------------------------------

/// Quantifier kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Quantifier {
    Exists,
    Forall,
}

/// Arithmetic comparison kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArithCmp {
    Eq,
    Leq,
    Lt,
}

/// An atomic formula.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Atom<C> {
    Arith(ArithCmp, ArithTerm<C>, ArithTerm<C>),
    ArrEq(ArrTerm<C>, ArrTerm<C>),
}

/// A proposition (boolean variable or boolean-valued application).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Proposition<C> {
    Var(u32),
    App(Symbol, Vec<Expr<C>>),
}

/// Destructured view of an arithmetic term.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArithTermView<C> {
    Real(QQ),
    App(Symbol, Vec<Expr<C>>),
    Var(u32, TypArith),
    Add(Vec<ArithTerm<C>>),
    Mul(Vec<ArithTerm<C>>),
    Div(ArithTerm<C>, ArithTerm<C>),
    Mod(ArithTerm<C>, ArithTerm<C>),
    Floor(ArithTerm<C>),
    Neg(ArithTerm<C>),
    Ite(Formula<C>, ArithTerm<C>, ArithTerm<C>),
    Select(ArrTerm<C>, ArithTerm<C>),
}

/// Destructured view of an array term.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrTermView<C> {
    App(Symbol, Vec<Expr<C>>),
    Var(u32),
    Store(ArrTerm<C>, ArithTerm<C>, ArithTerm<C>),
    Ite(Formula<C>, ArrTerm<C>, ArrTerm<C>),
}

/// Destructured view of a formula.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormulaView<C> {
    True,
    False,
    And(Vec<Formula<C>>),
    Or(Vec<Formula<C>>),
    Not(Formula<C>),
    Quantify(Quantifier, String, TypFo, Formula<C>),
    Atom(Atom<C>),
    Proposition(Proposition<C>),
    Ite(Formula<C>, Formula<C>, Formula<C>),
}
