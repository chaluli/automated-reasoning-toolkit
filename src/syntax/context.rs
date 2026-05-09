use std::marker::PhantomData;

use hashbrown::{HashMap, HashSet};

use crate::num::QQ;

use super::expr::{
    ArithCmp, ArithTerm, ArithTermView, ArrTerm, ArrTermView, Atom, Expr, ExprId, Formula,
    FormulaView, Proposition, Quantifier, SExprNode, Term,
};
use super::symbol::{Symbol, SymbolSet};
use super::types::{Typ, TypArith, TypFo, TypTerm};

/// A context manages symbols and expression sharing.
///
/// The type parameter `C` is a phantom marker that prevents expressions from
/// different contexts from being mixed at compile time.
#[derive(Debug)]
pub struct Context<C> {
    /// Arena of interned expression nodes.
    arena: Vec<SExprNode>,
    /// Hash-consing map: node -> existing `ExprId`.
    intern_map: HashMap<SExprNode, ExprId>,
    /// Symbol table: `symbol index -> (name, type)`.
    symbols: Vec<(String, Typ)>,
    /// Named symbol registry: `name -> symbol`.
    named_symbols: HashMap<String, Symbol>,
    /// Phantom type marker.
    _marker: PhantomData<C>,
}

impl<C> Context<C> {
    /// Create a new, empty context.
    pub fn new() -> Self {
        Self {
            arena: Vec::new(),
            intern_map: HashMap::new(),
            symbols: Vec::new(),
            named_symbols: HashMap::new(),
            _marker: PhantomData,
        }
    }

    /// Returns (number of expressions, number of symbols, number of named symbols).
    pub fn stats(&self) -> (usize, usize, usize) {
        (
            self.arena.len(),
            self.symbols.len(),
            self.named_symbols.len(),
        )
    }

    // -----------------------------------------------------------------------
    // Safe internal accessors
    // -----------------------------------------------------------------------

    /// Get an arena node by ExprId. All ExprIds are created by this context,
    /// so this is always valid.
    fn get_node(&self, id: ExprId) -> &SExprNode {
        #[expect(
            clippy::indexing_slicing,
            reason = "ExprId is always a valid arena index created by this context"
        )]
        &self.arena[id.0 as usize]
    }

    /// Get a symbol entry. All Symbols are created by this context's mk_symbol.
    fn get_symbol_entry(&self, sym: Symbol) -> &(String, Typ) {
        #[expect(
            clippy::indexing_slicing,
            reason = "Symbol is always a valid index created by this context"
        )]
        &self.symbols[sym.0 as usize]
    }

    /// Get a slice element by index (for mk_add/mk_or etc. with len==1).
    fn first_of<T: Copy>(slice: &[T]) -> T {
        #[expect(clippy::indexing_slicing, reason = "caller checks len >= 1")]
        slice[0]
    }

    // -----------------------------------------------------------------------
    // Interning
    // -----------------------------------------------------------------------

    /// Intern a node: if it already exists, return the existing ID; otherwise
    /// allocate a new slot in the arena.
    fn intern(&mut self, node: SExprNode) -> ExprId {
        if let Some(&id) = self.intern_map.get(&node) {
            return id;
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "arena size is bounded well below u32::MAX in practice"
        )]
        let id = ExprId(self.arena.len() as u32);
        self.arena.push(node.clone());
        self.intern_map.insert(node, id);
        id
    }

    /// Look up the node for a given `ExprId`.
    pub(crate) fn node(&self, id: ExprId) -> &SExprNode {
        self.get_node(id)
    }

    /// Public (crate-internal) interning entry point for substitution etc.
    pub(crate) fn intern_node(&mut self, node: SExprNode) -> ExprId {
        self.intern(node)
    }

    /// Classify an `ExprId` into a typed `Expr` handle (crate-internal).
    pub(crate) fn id_to_expr_pub(&self, id: ExprId) -> Expr<C> {
        self.id_to_expr(id)
    }

    // -----------------------------------------------------------------------
    // Helper: wrap ExprId into typed handles
    // -----------------------------------------------------------------------

    fn arith(&self, id: ExprId) -> ArithTerm<C> {
        ArithTerm {
            id,
            _phantom: PhantomData,
        }
    }

    fn arr(&self, id: ExprId) -> ArrTerm<C> {
        ArrTerm {
            id,
            _phantom: PhantomData,
        }
    }

    fn formula(&self, id: ExprId) -> Formula<C> {
        Formula {
            id,
            _phantom: PhantomData,
        }
    }

    // -----------------------------------------------------------------------
    // Symbols
    // -----------------------------------------------------------------------

    /// Create a fresh symbol with the given name and type.
    ///
    /// Multiple calls with the same name and type produce distinct symbols.
    pub fn mk_symbol(&mut self, name: &str, typ: Typ) -> Symbol {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "symbol count bounded well below u32::MAX"
        )]
        let idx = self.symbols.len() as u32;
        self.symbols.push((name.to_string(), typ));
        Symbol(idx)
    }

    /// Register a named symbol. The name must be unique within this context.
    ///
    /// Returns the new symbol, or `Err` if the name is already registered.
    pub fn register_named_symbol(
        &mut self,
        name: &str,
        typ: Typ,
    ) -> Result<Symbol, NameAlreadyRegistered> {
        if self.named_symbols.contains_key(name) {
            return Err(NameAlreadyRegistered(name.to_string()));
        }
        let sym = self.mk_symbol(name, typ);
        self.named_symbols.insert(name.to_string(), sym);
        Ok(sym)
    }

    /// Check whether a name is already registered.
    pub fn is_registered_name(&self, name: &str) -> bool {
        self.named_symbols.contains_key(name)
    }

    /// Retrieve the symbol associated with a name. Returns `None` if not registered.
    pub fn get_named_symbol(&self, name: &str) -> Option<Symbol> {
        self.named_symbols.get(name).copied()
    }

    /// Get the name of a symbol. Returns `None` for ordinary (non-named) symbols.
    pub fn symbol_name(&self, sym: Symbol) -> Option<&str> {
        let name = &self.get_symbol_entry(sym).0;
        if self.named_symbols.contains_key(name.as_str()) {
            Some(name)
        } else {
            None
        }
    }

    /// Get the display name of a symbol (always available, unlike `symbol_name`).
    pub fn show_symbol(&self, sym: Symbol) -> &str {
        &self.get_symbol_entry(sym).0
    }

    /// Get the type of a symbol.
    pub fn typ_symbol(&self, sym: Symbol) -> &Typ {
        &self.get_symbol_entry(sym).1
    }

    /// Create a fresh symbol with the same name and type as `sym`.
    pub fn dup_symbol(&mut self, sym: Symbol) -> Symbol {
        let entry = self.get_symbol_entry(sym).clone();
        self.mk_symbol(&entry.0, entry.1)
    }

    // -----------------------------------------------------------------------
    // Term constructors
    // -----------------------------------------------------------------------

    /// Create a rational literal.
    pub fn mk_real(&mut self, q: QQ) -> ArithTerm<C> {
        let id = self.intern(SExprNode::Real(q));
        self.arith(id)
    }

    /// Create an integer literal from a 64-bit int.
    pub fn mk_int(&mut self, n: i64) -> ArithTerm<C> {
        self.mk_real(QQ::of_i64(n))
    }

    /// The zero constant.
    pub fn mk_zero(&mut self) -> ArithTerm<C> {
        self.mk_real(QQ::zero())
    }

    /// The one constant.
    pub fn mk_one(&mut self) -> ArithTerm<C> {
        self.mk_real(QQ::one())
    }

    /// Create a sum. An empty list yields zero.
    pub fn mk_add(&mut self, terms: &[ArithTerm<C>]) -> ArithTerm<C> {
        match terms.len() {
            0 => self.mk_zero(),
            1 => Self::first_of(terms),
            _ => {
                let ids: Vec<ExprId> = terms.iter().map(|t| t.id).collect();
                let id = self.intern(SExprNode::Add(ids));
                self.arith(id)
            }
        }
    }

    /// Create a product. An empty list yields one.
    pub fn mk_mul(&mut self, terms: &[ArithTerm<C>]) -> ArithTerm<C> {
        match terms.len() {
            0 => self.mk_one(),
            1 => Self::first_of(terms),
            _ => {
                let ids: Vec<ExprId> = terms.iter().map(|t| t.id).collect();
                let id = self.intern(SExprNode::Mul(ids));
                self.arith(id)
            }
        }
    }

    /// Real-valued division.
    pub fn mk_div(&mut self, a: ArithTerm<C>, b: ArithTerm<C>) -> ArithTerm<C> {
        let id = self.intern(SExprNode::Div(a.id, b.id));
        self.arith(id)
    }

    /// C99 integer division: `truncate(a/b)`.
    pub fn mk_idiv(&mut self, a: ArithTerm<C>, b: ArithTerm<C>) -> ArithTerm<C> {
        let div = self.mk_div(a, b);
        self.mk_floor(div)
    }

    /// Modulo.
    pub fn mk_mod(&mut self, a: ArithTerm<C>, b: ArithTerm<C>) -> ArithTerm<C> {
        let id = self.intern(SExprNode::Mod(a.id, b.id));
        self.arith(id)
    }

    /// Unary negation.
    pub fn mk_neg(&mut self, a: ArithTerm<C>) -> ArithTerm<C> {
        let id = self.intern(SExprNode::Neg(a.id));
        self.arith(id)
    }

    /// Subtraction: `a - b`.
    pub fn mk_sub(&mut self, a: ArithTerm<C>, b: ArithTerm<C>) -> ArithTerm<C> {
        let neg_b = self.mk_neg(b);
        self.mk_add(&[a, neg_b])
    }

    /// Floor (round toward negative infinity).
    pub fn mk_floor(&mut self, a: ArithTerm<C>) -> ArithTerm<C> {
        let id = self.intern(SExprNode::Floor(a.id));
        self.arith(id)
    }

    /// Ceiling: ceil(a) = -floor(-a).
    pub fn mk_ceiling(&mut self, a: ArithTerm<C>) -> ArithTerm<C> {
        let neg = self.mk_neg(a);
        let floor = self.mk_floor(neg);
        self.mk_neg(floor)
    }

    /// Truncate: remove fractional part, rounding toward zero.
    pub fn mk_truncate(&mut self, a: ArithTerm<C>) -> ArithTerm<C> {
        let zero = self.mk_zero();
        let geq_zero = self.mk_leq(zero, a);
        let floor = self.mk_floor(a);
        let ceil = self.mk_ceiling(a);
        self.mk_arith_ite(geq_zero, floor, ceil)
    }

    /// Power: `a^n` for non-negative integer exponent.
    pub fn mk_pow(&mut self, a: ArithTerm<C>, n: u32) -> ArithTerm<C> {
        if n == 0 {
            return self.mk_one();
        }
        let factors = vec![a; n as usize];
        self.mk_mul(&factors)
    }

    // -----------------------------------------------------------------------
    // Array constructors
    // -----------------------------------------------------------------------

    /// Array select: `a[i]`.
    pub fn mk_select(&mut self, arr: ArrTerm<C>, idx: ArithTerm<C>) -> ArithTerm<C> {
        let id = self.intern(SExprNode::Select(arr.id, idx.id));
        self.arith(id)
    }

    /// Array store: `a[i := v]`.
    pub fn mk_store(
        &mut self,
        arr: ArrTerm<C>,
        idx: ArithTerm<C>,
        val: ArithTerm<C>,
    ) -> ArrTerm<C> {
        let id = self.intern(SExprNode::Store(arr.id, idx.id, val.id));
        self.arr(id)
    }

    // -----------------------------------------------------------------------
    // Formula constructors
    // -----------------------------------------------------------------------

    /// The constant `true`.
    pub fn mk_true(&mut self) -> Formula<C> {
        let id = self.intern(SExprNode::True);
        self.formula(id)
    }

    /// The constant `false`.
    pub fn mk_false(&mut self) -> Formula<C> {
        let id = self.intern(SExprNode::False);
        self.formula(id)
    }

    /// Conjunction. An empty list yields `true`.
    pub fn mk_and(&mut self, formulas: &[Formula<C>]) -> Formula<C> {
        match formulas.len() {
            0 => self.mk_true(),
            1 => Self::first_of(formulas),
            _ => {
                let ids: Vec<ExprId> = formulas.iter().map(|f| f.id).collect();
                let id = self.intern(SExprNode::And(ids));
                self.formula(id)
            }
        }
    }

    /// Disjunction. An empty list yields `false`.
    pub fn mk_or(&mut self, formulas: &[Formula<C>]) -> Formula<C> {
        match formulas.len() {
            0 => self.mk_false(),
            1 => Self::first_of(formulas),
            _ => {
                let ids: Vec<ExprId> = formulas.iter().map(|f| f.id).collect();
                let id = self.intern(SExprNode::Or(ids));
                self.formula(id)
            }
        }
    }

    /// Negation.
    pub fn mk_not(&mut self, a: Formula<C>) -> Formula<C> {
        let id = self.intern(SExprNode::Not(a.id));
        self.formula(id)
    }

    /// Arithmetic equality: `a = b`.
    pub fn mk_eq(&mut self, a: ArithTerm<C>, b: ArithTerm<C>) -> Formula<C> {
        let id = self.intern(SExprNode::Eq(a.id, b.id));
        self.formula(id)
    }

    /// Less-than: `a < b`.
    pub fn mk_lt(&mut self, a: ArithTerm<C>, b: ArithTerm<C>) -> Formula<C> {
        let id = self.intern(SExprNode::Lt(a.id, b.id));
        self.formula(id)
    }

    /// Less-than-or-equal: `a <= b`.
    pub fn mk_leq(&mut self, a: ArithTerm<C>, b: ArithTerm<C>) -> Formula<C> {
        let id = self.intern(SExprNode::Leq(a.id, b.id));
        self.formula(id)
    }

    /// Array equality.
    pub fn mk_arr_eq(&mut self, a: ArrTerm<C>, b: ArrTerm<C>) -> Formula<C> {
        let id = self.intern(SExprNode::ArrEq(a.id, b.id));
        self.formula(id)
    }

    /// Build an arithmetic comparison atom from an `ArithCmp` tag.
    pub fn mk_compare(&mut self, cmp: ArithCmp, a: ArithTerm<C>, b: ArithTerm<C>) -> Formula<C> {
        match cmp {
            ArithCmp::Eq => self.mk_eq(a, b),
            ArithCmp::Leq => self.mk_leq(a, b),
            ArithCmp::Lt => self.mk_lt(a, b),
        }
    }

    /// Universal quantification (de Bruijn).
    pub fn mk_forall(&mut self, name: &str, typ: TypFo, body: Formula<C>) -> Formula<C> {
        let id = self.intern(SExprNode::Forall(name.to_string(), typ, body.id));
        self.formula(id)
    }

    /// Existential quantification (de Bruijn).
    pub fn mk_exists(&mut self, name: &str, typ: TypFo, body: Formula<C>) -> Formula<C> {
        let id = self.intern(SExprNode::Exists(name.to_string(), typ, body.id));
        self.formula(id)
    }

    /// Implication: `a => b` is `!a || b`.
    pub fn mk_if(&mut self, a: Formula<C>, b: Formula<C>) -> Formula<C> {
        let not_a = self.mk_not(a);
        self.mk_or(&[not_a, b])
    }

    /// Bi-implication: `a <=> b` is `(a => b) && (b => a)`.
    pub fn mk_iff(&mut self, a: Formula<C>, b: Formula<C>) -> Formula<C> {
        let fwd = self.mk_if(a, b);
        let bwd = self.mk_if(b, a);
        self.mk_and(&[fwd, bwd])
    }

    // -----------------------------------------------------------------------
    // Generic constructors
    // -----------------------------------------------------------------------

    /// Create a constant symbol expression.
    pub fn mk_const(&mut self, sym: Symbol) -> Expr<C> {
        let id = self.intern(SExprNode::App(sym, Vec::new()));
        self.id_to_expr(id)
    }

    /// Create a function application.
    pub fn mk_app(&mut self, sym: Symbol, args: &[Expr<C>]) -> Expr<C> {
        let ids: Vec<ExprId> = args.iter().map(|e| e.id()).collect();
        let id = self.intern(SExprNode::App(sym, ids));
        self.id_to_expr(id)
    }

    /// Create a de Bruijn variable.
    pub fn mk_var(&mut self, index: u32, typ: TypFo) -> Expr<C> {
        let id = self.intern(SExprNode::Var(index, typ));
        self.id_to_expr(id)
    }

    /// If-then-else for formulas.
    pub fn mk_formula_ite(
        &mut self,
        cond: Formula<C>,
        then_: Formula<C>,
        else_: Formula<C>,
    ) -> Formula<C> {
        let id = self.intern(SExprNode::Ite(cond.id, then_.id, else_.id));
        self.formula(id)
    }

    /// If-then-else for arithmetic terms.
    pub fn mk_arith_ite(
        &mut self,
        cond: Formula<C>,
        then_: ArithTerm<C>,
        else_: ArithTerm<C>,
    ) -> ArithTerm<C> {
        let id = self.intern(SExprNode::Ite(cond.id, then_.id, else_.id));
        self.arith(id)
    }

    /// If-then-else for array terms.
    pub fn mk_arr_ite(
        &mut self,
        cond: Formula<C>,
        then_: ArrTerm<C>,
        else_: ArrTerm<C>,
    ) -> ArrTerm<C> {
        let id = self.intern(SExprNode::Ite(cond.id, then_.id, else_.id));
        self.arr(id)
    }

    // -----------------------------------------------------------------------
    // Type queries
    // -----------------------------------------------------------------------

    /// Get the first-order type of an expression.
    pub fn expr_typ(&self, expr: &Expr<C>) -> TypFo {
        self.id_typ(expr.id())
    }

    fn id_typ(&self, id: ExprId) -> TypFo {
        match self.get_node(id) {
            SExprNode::Real(_) => TypFo::Real,
            SExprNode::Floor(_) => TypFo::Int,
            SExprNode::Neg(c) => self.id_typ(*c),
            SExprNode::Add(cs) | SExprNode::Mul(cs) => {
                if cs.iter().any(|c| self.id_typ(*c) == TypFo::Real) {
                    TypFo::Real
                } else {
                    TypFo::Int
                }
            }
            SExprNode::Div(..) => TypFo::Real,
            SExprNode::Mod(a, _) => self.id_typ(*a),
            SExprNode::Select(..) => TypFo::Real,
            SExprNode::Var(_, typ) => *typ,
            SExprNode::App(sym, _) => match self.typ_symbol(*sym) {
                Typ::Fun(_, cod) => *cod,
                Typ::Int => TypFo::Int,
                Typ::Real => TypFo::Real,
                Typ::Bool => TypFo::Bool,
                Typ::Arr => TypFo::Arr,
            },
            SExprNode::Store(..) => TypFo::Arr,
            SExprNode::True
            | SExprNode::False
            | SExprNode::And(_)
            | SExprNode::Or(_)
            | SExprNode::Not(_)
            | SExprNode::Eq(..)
            | SExprNode::Leq(..)
            | SExprNode::Lt(..)
            | SExprNode::ArrEq(..)
            | SExprNode::Exists(..)
            | SExprNode::Forall(..) => TypFo::Bool,
            SExprNode::Ite(_, then_branch, _) => self.id_typ(*then_branch),
        }
    }

    // -----------------------------------------------------------------------
    // Classify an ExprId into a typed handle
    // -----------------------------------------------------------------------

    fn id_to_expr(&self, id: ExprId) -> Expr<C> {
        match self.id_typ(id) {
            TypFo::Int | TypFo::Real => Expr::ArithTerm(self.arith(id)),
            TypFo::Arr => Expr::ArrTerm(self.arr(id)),
            TypFo::Bool => Expr::Formula(self.formula(id)),
        }
    }

    // -----------------------------------------------------------------------
    // Expression size & free variables
    // -----------------------------------------------------------------------

    /// Count the number of unique sub-expression nodes in `expr`.
    pub fn size(&self, expr: &Expr<C>) -> usize {
        let mut visited = HashSet::new();
        self.size_rec(expr.id(), &mut visited)
    }

    fn size_rec(&self, id: ExprId, visited: &mut HashSet<ExprId>) -> usize {
        if !visited.insert(id) {
            return 1;
        }
        let children_size: usize = self
            .children(id)
            .iter()
            .map(|c| self.size_rec(*c, visited))
            .sum();
        1 + children_size
    }

    fn children(&self, id: ExprId) -> Vec<ExprId> {
        match self.get_node(id) {
            SExprNode::Real(_) | SExprNode::True | SExprNode::False | SExprNode::Var(..) => {
                Vec::new()
            }
            SExprNode::Add(cs) | SExprNode::Mul(cs) | SExprNode::And(cs) | SExprNode::Or(cs) => {
                cs.clone()
            }
            SExprNode::App(_, cs) => cs.clone(),
            SExprNode::Div(a, b)
            | SExprNode::Mod(a, b)
            | SExprNode::Eq(a, b)
            | SExprNode::Leq(a, b)
            | SExprNode::Lt(a, b)
            | SExprNode::ArrEq(a, b)
            | SExprNode::Select(a, b) => vec![*a, *b],
            SExprNode::Floor(a) | SExprNode::Neg(a) | SExprNode::Not(a) => vec![*a],
            SExprNode::Exists(_, _, a) | SExprNode::Forall(_, _, a) => vec![*a],
            SExprNode::Store(a, b, c) | SExprNode::Ite(a, b, c) => vec![*a, *b, *c],
        }
    }

    /// Collect the set of constant symbols that appear in `expr`.
    pub fn symbols(&self, expr: &Expr<C>) -> SymbolSet {
        let mut result = SymbolSet::new();
        self.symbols_rec(expr.id(), &mut result);
        result
    }

    fn symbols_rec(&self, id: ExprId, result: &mut SymbolSet) {
        if let SExprNode::App(sym, _) = self.get_node(id) {
            result.insert(*sym);
        }
        for child in &self.children(id) {
            self.symbols_rec(*child, result);
        }
    }

    /// Collect free de Bruijn variable indices and their types.
    pub fn free_vars(&self, expr: &Expr<C>) -> HashMap<u32, TypFo> {
        let mut result = HashMap::new();
        self.free_vars_rec(expr.id(), &mut result);
        result
    }

    fn free_vars_rec(&self, id: ExprId, result: &mut HashMap<u32, TypFo>) {
        if let SExprNode::Var(idx, typ) = self.get_node(id) {
            result.insert(*idx, *typ);
        }
        for child in &self.children(id) {
            self.free_vars_rec(*child, result);
        }
    }

    // -----------------------------------------------------------------------
    // View methods -- destructure into typed views for pattern matching
    // -----------------------------------------------------------------------

    /// Destructure an arithmetic term.
    pub fn view_arith_term(&self, t: ArithTerm<C>) -> ArithTermView<C> {
        match self.get_node(t.id) {
            SExprNode::Real(q) => ArithTermView::Real(q.clone()),
            SExprNode::App(sym, args) => {
                ArithTermView::App(*sym, args.iter().map(|id| self.id_to_expr(*id)).collect())
            }
            SExprNode::Var(idx, typ) => {
                let arith_typ = match typ {
                    TypFo::Int => TypArith::Int,
                    _ => TypArith::Real,
                };
                ArithTermView::Var(*idx, arith_typ)
            }
            SExprNode::Add(cs) => ArithTermView::Add(cs.iter().map(|id| self.arith(*id)).collect()),
            SExprNode::Mul(cs) => ArithTermView::Mul(cs.iter().map(|id| self.arith(*id)).collect()),
            SExprNode::Div(a, b) => ArithTermView::Div(self.arith(*a), self.arith(*b)),
            SExprNode::Mod(a, b) => ArithTermView::Mod(self.arith(*a), self.arith(*b)),
            SExprNode::Floor(a) => ArithTermView::Floor(self.arith(*a)),
            SExprNode::Neg(a) => ArithTermView::Neg(self.arith(*a)),
            SExprNode::Select(arr, idx) => ArithTermView::Select(self.arr(*arr), self.arith(*idx)),
            SExprNode::Ite(cond, then_, else_) => {
                ArithTermView::Ite(self.formula(*cond), self.arith(*then_), self.arith(*else_))
            }
            #[expect(
                clippy::unreachable,
                reason = "ArithTerm<C> is only constructed by mk_* APIs that intern arith-sorted SExprNodes; reaching this arm means an ill-typed handle was forged or the invariant is broken."
            )]
            _ => unreachable!("view_arith_term: non-arithmetic SExprNode under ArithTerm handle"),
        }
    }

    /// Destructure an array term.
    pub fn view_arr_term(&self, t: ArrTerm<C>) -> ArrTermView<C> {
        match self.get_node(t.id) {
            SExprNode::App(sym, args) => {
                ArrTermView::App(*sym, args.iter().map(|id| self.id_to_expr(*id)).collect())
            }
            SExprNode::Var(idx, _) => ArrTermView::Var(*idx),
            SExprNode::Store(arr, idx, val) => {
                ArrTermView::Store(self.arr(*arr), self.arith(*idx), self.arith(*val))
            }
            SExprNode::Ite(cond, then_, else_) => {
                ArrTermView::Ite(self.formula(*cond), self.arr(*then_), self.arr(*else_))
            }
            #[expect(
                clippy::unreachable,
                reason = "ArrTerm<C> is only constructed by mk_* APIs that intern array-sorted SExprNodes; reaching this arm means an ill-typed handle was forged or the invariant is broken."
            )]
            _ => unreachable!("view_arr_term: non-array SExprNode under ArrTerm handle"),
        }
    }

    /// Destructure a formula.
    pub fn view_formula(&self, f: Formula<C>) -> FormulaView<C> {
        match self.get_node(f.id) {
            SExprNode::True => FormulaView::True,
            SExprNode::False => FormulaView::False,
            SExprNode::And(cs) => FormulaView::And(cs.iter().map(|id| self.formula(*id)).collect()),
            SExprNode::Or(cs) => FormulaView::Or(cs.iter().map(|id| self.formula(*id)).collect()),
            SExprNode::Not(a) => FormulaView::Not(self.formula(*a)),
            SExprNode::Exists(name, typ, body) => {
                FormulaView::Quantify(Quantifier::Exists, name.clone(), *typ, self.formula(*body))
            }
            SExprNode::Forall(name, typ, body) => {
                FormulaView::Quantify(Quantifier::Forall, name.clone(), *typ, self.formula(*body))
            }
            SExprNode::Eq(a, b) => {
                FormulaView::Atom(Atom::Arith(ArithCmp::Eq, self.arith(*a), self.arith(*b)))
            }
            SExprNode::Leq(a, b) => {
                FormulaView::Atom(Atom::Arith(ArithCmp::Leq, self.arith(*a), self.arith(*b)))
            }
            SExprNode::Lt(a, b) => {
                FormulaView::Atom(Atom::Arith(ArithCmp::Lt, self.arith(*a), self.arith(*b)))
            }
            SExprNode::ArrEq(a, b) => FormulaView::Atom(Atom::ArrEq(self.arr(*a), self.arr(*b))),
            SExprNode::App(sym, args) => {
                let exprs: Vec<Expr<C>> = args.iter().map(|id| self.id_to_expr(*id)).collect();
                FormulaView::Proposition(Proposition::App(*sym, exprs))
            }
            SExprNode::Var(idx, _) => FormulaView::Proposition(Proposition::Var(*idx)),
            SExprNode::Ite(cond, then_, else_) => FormulaView::Ite(
                self.formula(*cond),
                self.formula(*then_),
                self.formula(*else_),
            ),
            #[expect(
                clippy::unreachable,
                reason = "Formula<C> is only constructed by mk_* APIs that intern bool-sorted SExprNodes; reaching this arm means an ill-typed handle was forged or the invariant is broken."
            )]
            _ => unreachable!("view_formula: non-boolean SExprNode under Formula handle"),
        }
    }

    /// Get the `TypArith` for an arithmetic term.
    pub fn arith_term_typ(&self, t: ArithTerm<C>) -> TypArith {
        match self.id_typ(t.id) {
            TypFo::Int => TypArith::Int,
            _ => TypArith::Real,
        }
    }

    /// Get the `TypTerm` for a generic term.
    pub fn term_typ(&self, t: &Term<C>) -> TypTerm {
        match t {
            Term::Arith(a) => match self.arith_term_typ(*a) {
                TypArith::Int => TypTerm::Int,
                TypArith::Real => TypTerm::Real,
            },
            Term::Arr(_) => TypTerm::Arr,
        }
    }
}

impl<C> Default for Context<C> {
    fn default() -> Self {
        Self::new()
    }
}

/// Error returned when attempting to register a name that is already in use.
#[derive(Debug, Clone)]
pub struct NameAlreadyRegistered(pub String);

impl std::fmt::Display for NameAlreadyRegistered {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "register_named_symbol: the name '{}' has already been registered",
            self.0
        )
    }
}

impl std::error::Error for NameAlreadyRegistered {}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    enum TestCtx {}

    #[test]
    fn test_hash_consing() {
        let mut ctx = Context::<TestCtx>::new();
        let a = ctx.mk_int(42);
        let b = ctx.mk_int(42);
        assert_eq!(a, b, "same literal should yield same handle");
    }

    #[test]
    fn test_hash_consing_compound() {
        let mut ctx = Context::<TestCtx>::new();
        let x = ctx.mk_int(1);
        let y = ctx.mk_int(2);
        let sum1 = ctx.mk_add(&[x, y]);
        let sum2 = ctx.mk_add(&[x, y]);
        assert_eq!(sum1, sum2, "same structure should yield same handle");
    }

    #[test]
    fn test_mk_and_or_empty() {
        let mut ctx = Context::<TestCtx>::new();
        let t = ctx.mk_true();
        let f = ctx.mk_false();
        assert_eq!(ctx.mk_and(&[]), t);
        assert_eq!(ctx.mk_or(&[]), f);
    }

    #[test]
    fn test_view_arith_roundtrip() {
        let mut ctx = Context::<TestCtx>::new();
        let a = ctx.mk_int(3);
        let b = ctx.mk_int(5);
        let sum = ctx.mk_add(&[a, b]);
        let view = ctx.view_arith_term(sum);
        assert_eq!(view, ArithTermView::Add(vec![a, b]));
    }

    #[test]
    fn test_view_formula_roundtrip() {
        let mut ctx = Context::<TestCtx>::new();
        let x = ctx.mk_int(1);
        let y = ctx.mk_int(2);
        let eq = ctx.mk_eq(x, y);
        let view = ctx.view_formula(eq);
        assert_eq!(view, FormulaView::Atom(Atom::Arith(ArithCmp::Eq, x, y)));
    }

    #[test]
    fn test_symbols_collection() {
        let mut ctx = Context::<TestCtx>::new();
        let s = ctx.mk_symbol("x", Typ::Real);
        let x = ctx.mk_const(s);
        let syms = ctx.symbols(&x);
        assert!(syms.contains(&s));
    }

    #[test]
    fn test_named_symbol() {
        let mut ctx = Context::<TestCtx>::new();
        let result = ctx.register_named_symbol("x", Typ::Int);
        assert!(result.is_ok());
        let sym = ctx.get_named_symbol("x");
        assert!(sym.is_some());
        // Use if-let to avoid unwrap
        if let Some(s) = sym {
            assert_eq!(ctx.symbol_name(s), Some("x"));
        }

        // Duplicate should fail
        let dup = ctx.register_named_symbol("x", Typ::Int);
        assert!(dup.is_err());
    }
}
