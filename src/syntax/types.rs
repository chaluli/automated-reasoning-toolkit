use std::fmt;

/// First-order types (used in quantifiers, variables, function arguments).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TypFo {
    Int,
    Real,
    Bool,
    Arr,
}

/// Arithmetic types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TypArith {
    Int,
    Real,
}

/// Array type marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TypArr;

/// Term types (arithmetic or array).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TypTerm {
    Int,
    Real,
    Arr,
}

/// Full type (includes function types).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Typ {
    Int,
    Real,
    Bool,
    Arr,
    Fun(Vec<TypFo>, TypFo),
}

// --- From conversions: sub-types widen into super-types ---

impl From<TypArith> for TypFo {
    fn from(t: TypArith) -> Self {
        match t {
            TypArith::Int => TypFo::Int,
            TypArith::Real => TypFo::Real,
        }
    }
}

impl From<TypArr> for TypFo {
    fn from(_: TypArr) -> Self {
        TypFo::Arr
    }
}

impl From<TypTerm> for TypFo {
    fn from(t: TypTerm) -> Self {
        match t {
            TypTerm::Int => TypFo::Int,
            TypTerm::Real => TypFo::Real,
            TypTerm::Arr => TypFo::Arr,
        }
    }
}

impl From<TypArith> for TypTerm {
    fn from(t: TypArith) -> Self {
        match t {
            TypArith::Int => TypTerm::Int,
            TypArith::Real => TypTerm::Real,
        }
    }
}

impl From<TypArr> for TypTerm {
    fn from(_: TypArr) -> Self {
        TypTerm::Arr
    }
}

impl From<TypFo> for Typ {
    fn from(t: TypFo) -> Self {
        match t {
            TypFo::Int => Typ::Int,
            TypFo::Real => Typ::Real,
            TypFo::Bool => Typ::Bool,
            TypFo::Arr => Typ::Arr,
        }
    }
}

impl From<TypArith> for Typ {
    fn from(t: TypArith) -> Self {
        match t {
            TypArith::Int => Typ::Int,
            TypArith::Real => Typ::Real,
        }
    }
}

impl From<TypArr> for Typ {
    fn from(_: TypArr) -> Self {
        Typ::Arr
    }
}

impl From<TypTerm> for Typ {
    fn from(t: TypTerm) -> Self {
        match t {
            TypTerm::Int => Typ::Int,
            TypTerm::Real => Typ::Real,
            TypTerm::Arr => Typ::Arr,
        }
    }
}

/// Returns true if `sub` is a subtype of `sup` (Int <: Real).
pub fn is_subtype(sub: TypFo, sup: TypFo) -> bool {
    sub == sup || (sub == TypFo::Int && sup == TypFo::Real)
}

impl fmt::Display for TypFo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypFo::Int => write!(f, "int"),
            TypFo::Real => write!(f, "real"),
            TypFo::Bool => write!(f, "bool"),
            TypFo::Arr => write!(f, "array"),
        }
    }
}

impl fmt::Display for TypArith {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypArith::Int => write!(f, "int"),
            TypArith::Real => write!(f, "real"),
        }
    }
}

impl fmt::Display for TypArr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "array")
    }
}

impl fmt::Display for TypTerm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypTerm::Int => write!(f, "int"),
            TypTerm::Real => write!(f, "real"),
            TypTerm::Arr => write!(f, "array"),
        }
    }
}

impl fmt::Display for Typ {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Typ::Int => write!(f, "int"),
            Typ::Real => write!(f, "real"),
            Typ::Bool => write!(f, "bool"),
            Typ::Arr => write!(f, "array"),
            Typ::Fun(dom, cod) => {
                write!(f, "(")?;
                for (i, t) in dom.iter().enumerate() {
                    if i > 0 {
                        write!(f, " * ")?;
                    }
                    write!(f, "{t}")?;
                }
                write!(f, " -> {cod})")
            }
        }
    }
}
