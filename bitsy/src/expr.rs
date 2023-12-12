use reference::Reference;
mod typecheck;

use super::*;
use crate::sim::NetId;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use once_cell::sync::OnceCell;


/// An expression.
#[derive(Clone)]
pub enum Expr {
    /// A referenec to a port, reg, or node.
    Reference(Loc, OnceCell<Arc<Type>>, Path),
    /// A referenec to a net. Used only in [`crate::sim::Sim`]. See [`Expr::references_to_nets`].
    Net(Loc, OnceCell<Arc<Type>>, NetId),
    /// A literal Word.
    Word(Loc, OnceCell<Arc<Type>>, Option<Width>, u64),
    /// A literal enum value.
    Enum(Loc, OnceCell<Arc<Type>>, Reference<TypeDef>, String),
    /// Constructor (for `Valid<T>`)
    Ctor(Loc, OnceCell<Arc<Type>>, String, Vec<Arc<Expr>>),
    /// Let binding. Eg, `let x = a + b in x + x`.
    Let(Loc, OnceCell<Arc<Type>>, String, Arc<Expr>, Arc<Expr>),
    /// A unary operation. Eg, `!0b101w3`.
    UnOp(Loc, OnceCell<Arc<Type>>, UnOp, Arc<Expr>),
    /// A binary operation. Eg, `1w8 + 1w8`.
    BinOp(Loc, OnceCell<Arc<Type>>, BinOp, Arc<Expr>, Arc<Expr>),
    /// An `if` expression.
    If(Loc, OnceCell<Arc<Type>>, Arc<Expr>, Arc<Expr>, Arc<Expr>),
    /// A `match` expression.
    Match(Loc, OnceCell<Arc<Type>>, Arc<Expr>, Vec<MatchArm>),
    /// A multiplexer. Eg, `mux(cond, a, b)`.
    Mux(Loc, OnceCell<Arc<Type>>, Arc<Expr>, Arc<Expr>, Arc<Expr>),
    /// A concatenate expression. Eg, `cat(foo, 0w1)`.
    Cat(Loc, OnceCell<Arc<Type>>, Vec<Arc<Expr>>),
    /// A sign extension expression.
    Sext(Loc, OnceCell<Arc<Type>>, Arc<Expr>),
    /// A word expression. Used to cast user-defined `enum` types to their bit values.
    ToWord(Loc, OnceCell<Arc<Type>>, Arc<Expr>),
    /// A vector constructor expression. Eg, `[0w2, 1w2, 2w2]`.
    Vec(Loc, OnceCell<Arc<Type>>, Vec<Arc<Expr>>),
    /// A static index. Eg, `foo[0]`.
    Idx(Loc, OnceCell<Arc<Type>>, Arc<Expr>, u64),
    IdxRange(Loc, OnceCell<Arc<Type>>, Arc<Expr>, u64, u64),
//    /// A static index range. Eg, `foo[8..4]`.
//    IdxDyn(Loc, OnceCell<Arc<Type>>, Arc<Expr>, Arc<Expr>),
    /// A hole. Eg, `?foo`.
    Hole(Loc, OnceCell<Arc<Type>>, Option<String>),
}

#[derive(Clone, Debug)]
pub struct MatchArm(pub Pat, pub Arc<Expr>);

#[derive(Clone, Debug)]
pub enum Pat {
    At(String, Vec<Pat>),
    Bind(String),
    Otherwise,
}

impl HasLoc for Expr {
    fn loc(&self) -> Loc {
        match self {
            Expr::Net(loc, _typ, _netid) => loc.clone(),
            Expr::Reference(loc, _typ, _path) => loc.clone(),
            Expr::Word(loc, _typ, _width, _val) => loc.clone(),
            Expr::Enum(loc, _typ, _typedef, _name) => loc.clone(),
            Expr::Ctor(loc, _typ, _name, _e) => loc.clone(),
            Expr::Let(loc, _typ, _name, _e, _b) => loc.clone(),
            Expr::UnOp(loc, _typ, _op, _e) => loc.clone(),
            Expr::BinOp(loc, _typ, _op, _e1, _e2) => loc.clone(),
            Expr::If(loc, _typ, _cond, _e1, _e2) => loc.clone(),
            Expr::Match(loc, _typ, _e, _arms) => loc.clone(),
            Expr::Mux(loc, _typ, _cond, _e1, _e2) => loc.clone(),
            Expr::Cat(loc, _typ, _es) => loc.clone(),
            Expr::Sext(loc, _typ, _e) => loc.clone(),
            Expr::ToWord(loc, _typ, _e) => loc.clone(),
            Expr::Vec(loc, _typ, _es) => loc.clone(),
            Expr::Idx(loc, _typ, _e, _i) => loc.clone(),
            Expr::IdxRange(loc, _typ, _e, _j, _i) => loc.clone(),
//            Expr::IdxDyn(loc, _typ, _e, _i) => loc.clone(),
            Expr::Hole(loc, _typ, _opt_name) => loc.clone(),
        }
    }
}

impl HasLoc for Arc<Expr> {
    fn loc(&self) -> Loc {
        let e: &Expr = &*self;
        e.loc()
    }
}

impl std::fmt::Debug for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Expr::Net(_loc, _typ, netid) => write!(f, "#{netid:?}"),
            Expr::Reference(_loc, _typ, path) => write!(f, "{path}"),
            Expr::Word(_loc, _typ, None, val) => write!(f, "{val}"),
            Expr::Word(_loc, _typ, Some(width), val) => write!(f, "{val}w{width}"),
            Expr::Enum(_loc, _typ, typedef, name) => write!(f, "{typedef:?}::{name}"),
            Expr::Let(_loc, _typ, name, e, b) => write!(f, "let {name} = {e:?} {{ {b:?} }}"),
            Expr::Ctor(_loc, _typ, name, e) => write!(f, "@{name}({e:?})"),
            Expr::UnOp(_loc, _typ, op, e) => {
                let op_symbol = match op {
                    UnOp::Not => "!",
                };
                write!(f, "({op_symbol}{e:?})")
            },
            Expr::BinOp(_loc, _typ, op, e1, e2) => {
                let op_symbol = match op {
                    BinOp::Add => "+",
                    BinOp::AddCarry => "+%",
                    BinOp::Sub => "-",
//                    BinOp::SubBorrow => "-%",
                    BinOp::And => "&&",
                    BinOp::Or => "||",
                    BinOp::Xor => "^",
                    BinOp::Eq => "==",
                    BinOp::Neq => "!=",
                    BinOp::Lt => "<",
                };
                write!(f, "({e1:?} {op_symbol} {e2:?})")
            },
            Expr::If(_loc, _typ, cond, e1, e2) => {
                write!(f, "if {cond:?} {{ {e1:?} }} else {{ {e2:?} }}")
            },
            Expr::Match(_loc, _typ, e, arms) => {
                write!(f, "match {e:?} {{ ... }}") // TODO
            },
            Expr::Mux(_loc, _typ, cond, e1, e2) => write!(f, "mux({cond:?}, {e1:?}, {e2:?})"),
            Expr::Cat(_loc, _typ, es) => write!(f, "cat({})", es.iter().map(|e| format!("{e:?}")).collect::<Vec<_>>().join(", ")),
            Expr::Sext(_loc, _typ, e) => write!(f, "sext({e:?})"),
            Expr::ToWord(_loc, _typ, e) => write!(f, "word({e:?})"),
            Expr::Vec(_loc, _typ, es) => {
                write!(f, "[")?;
                for (i, e) in es.iter().enumerate() {
                    if i + 1 < es.len() {
                        write!(f, "{e:?}, ")?;
                    } else {
                        write!(f, "{e:?}")?;
                    }
                }
                write!(f, "]")
            },
            Expr::Idx(_loc, _typ, e, i) => write!(f, "{e:?}[{i}]"),
            Expr::IdxRange(_loc, _typ, e, j, i) => write!(f, "{e:?}[{j}..{i}]"),
//            Expr::IdxDyn(_loc, e, i) => write!(f, "{e:?}[{i:?}]"),
            Expr::Hole(_loc, _typ, opt_name) => {
                if let Some(name) = opt_name {
                    write!(f, "?{name}")
                } else {
                    write!(f, "?")
                }
            }
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum UnOp {
    Not,
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum BinOp {
    Add,
    AddCarry,
    Sub,
//    SubBorrow,
    And,
    Or,
    Xor,
    Eq,
    Neq,
    Lt,
}

impl Expr {
    pub fn assert_has_types(&self) {
        let mut func = |e: &Expr| {
            if let Expr::Word(_loc, typ, _width, _n) = e {
                typ.get().unwrap();
            }
        };
        self.with_subexprs(&mut func);
    }

    /// Walk the expression tree in-order, calling `callback` for each subexpression.
    pub fn with_subexprs(&self, callback: &mut dyn FnMut(&Expr)) {
        match self {
            Expr::Reference(_loc, _typ, _path) => callback(self),
            Expr::Net(_loc, _typ, _netid) => callback(self),
            Expr::Word(_loc, _typ, _width, _value) => callback(self),
            Expr::Enum(_loc, _typ, _typedef, _name) => callback(self),
            Expr::Ctor(_loc, _typ, _name, es) => {
                callback(self);
                for e in es {
                    e.with_subexprs(callback);
                }
            },
            Expr::Let(_loc, _typ, _name, e, b) => {
                callback(self);
                e.with_subexprs(callback);
                b.with_subexprs(callback);
            },
            Expr::UnOp(_loc, _typ, _op, e) => {
                callback(self);
                e.with_subexprs(callback);
            }
            Expr::BinOp(_loc, _typ, _op, e1, e2) => {
                callback(self);
                e1.with_subexprs(callback);
                e2.with_subexprs(callback);
            },
            Expr::If(_loc, _typ, cond, e1, e2) => {
                callback(self);
                cond.with_subexprs(callback);
                e1.with_subexprs(callback);
                e2.with_subexprs(callback);
            },
            Expr::Match(_loc, _typ, e, arms) => {
                callback(self);
                callback(e);
                for MatchArm(_pat, arm_e) in arms {
                    arm_e.with_subexprs(callback);
                }
            }
            Expr::Mux(_loc, _typ, cond, e1, e2) => {
                callback(self);
                cond.with_subexprs(callback);
                e1.with_subexprs(callback);
                e2.with_subexprs(callback);
            },
            Expr::Cat(_loc, _typ, es) => {
                callback(self);
                for e in es {
                    e.with_subexprs(callback);
                }
            },
            Expr::Sext(_loc, _typ, e) => {
                callback(self);
                e.with_subexprs(callback);
            },
            Expr::ToWord(_loc, _typ, e) => {
                callback(self);
                e.with_subexprs(callback);
            },
            Expr::Vec(_loc, _typ, es) => {
                callback(self);
                for e in es {
                    e.with_subexprs(callback);
                }
            },
            Expr::Idx(_loc, _typ, e, _i) => {
                callback(self);
                e.with_subexprs(callback);
            },
            Expr::IdxRange(_loc, _typ, e, _j, _i) => {
                callback(self);
                e.with_subexprs(callback);
            },
            /*Expr::IdxDyn(_loc, e, _i) => {
                callback(self);
                e.with_subexprs(callback);
            },*/
            Expr::Hole(_loc, _typ, _name) => {
                callback(self);
            },
        }
    }

    pub fn paths(&self) -> Vec<Path> {
        let paths = std::cell::RefCell::new(vec![]);
        let mut func = |e: &Expr| {
            if let Expr::Reference(_loc, _typ, path) = e {
                paths.borrow_mut().push(path.clone());
            } else if let Expr::Net(_loc, _typ, _netid) = e {
                panic!("paths() only works on symbolic expressions.");
            }
        };
        self.with_subexprs(&mut func);

        let mut results = paths.into_inner();
        results.sort();
        results.dedup();
        results
    }

    pub fn is_constant(&self) -> bool {
        self.free_vars().is_empty()
    }

    pub fn free_vars(&self) -> BTreeSet<Path> {
        match self {
            Expr::Reference(_loc, _typ, path) => vec![path.clone()].iter().cloned().collect(),
            Expr::Net(_loc, _typ, _netid) => BTreeSet::new(),
            Expr::Word(_loc, _typ, _width, _value) => BTreeSet::new(),
            Expr::Enum(_loc, _typ, _typedef, _name) => BTreeSet::new(),
            Expr::Ctor(_loc, _typ, _name, es) => {
                let mut result = BTreeSet::new();
                for e in es {
                    result.extend(e.free_vars())
                }
                result
            },
            Expr::Let(_loc, _typ, x, e, b) => {
                let mut result = b.free_vars();
                result.remove(&x.to_string().into());
                result.union(&e.free_vars()).cloned().collect()
            },
            Expr::UnOp(_loc, _typ, _op, e) => e.free_vars(),
            Expr::BinOp(_loc, _typ, _op, e1, e2) => e1.free_vars().union(&e2.free_vars()).cloned().collect(),
            Expr::If(_loc, _typ, cond, e1, e2) => {
                cond.free_vars()
                    .union(&e1.free_vars())
                    .cloned()
                    .collect::<BTreeSet<_>>()
                    .union(&e2.free_vars())
                    .cloned()
                    .collect()
            },
            Expr::Match(_loc, _typ, _e, arms) => {
                todo!()
            },
            Expr::Mux(_loc, _typ, cond, e1, e2) => {
                cond.free_vars()
                    .union(&e1.free_vars())
                    .cloned()
                    .collect::<BTreeSet<_>>()
                    .union(&e2.free_vars())
                    .cloned()
                    .collect()
            },
            Expr::Cat(_loc, _typ, es) => {
                let mut result = BTreeSet::new();
                for e in es {
                    result.extend(e.free_vars())
                }
                result
            },
            Expr::ToWord(_loc, _typ, e) => e.free_vars(),
            Expr::Vec(_loc, _typ, es) => {
                let mut result = BTreeSet::new();
                for e in es {
                    result.extend(e.free_vars())
                }
                result
            },
            Expr::Sext(_loc, _typ, e) => e.free_vars(),
            Expr::Idx(_loc, _typ, e, _i) => e.free_vars(),
            Expr::IdxRange(_loc, _typ, e, _j, _i) => e.free_vars(),
            //Expr::IdxDyn(_loc, e, i) => e.free_vars().union(&i.free_vars()).cloned().collect(),
            Expr::Hole(_loc, _typ, _name) => BTreeSet::new(),
        }
    }

    pub fn depends_on(&self, path: Path) -> bool {
        self.paths().contains(&path)
    }

    pub fn depends_on_net(&self, net_id: NetId) -> bool {
        match self {
            Expr::Reference(_loc, _typ, _path) => false,
            Expr::Net(_loc, _typ, other_netid) => net_id == *other_netid,
            Expr::Word(_loc, _typ, _width, _value) => false,
            Expr::Enum(_loc, _typ, _typedef, _name) => false,
            Expr::Ctor(_loc, _typ, _name, es) => es.iter().any(|e| e.depends_on_net(net_id)),
            Expr::Let(_loc, _typ, _name, e, b) => e.depends_on_net(net_id) || b.depends_on_net(net_id),
            Expr::Match(_loc, _typ, e, arms) => e.depends_on_net(net_id) || arms.iter().any(|MatchArm(_pat, arm_e)| arm_e.depends_on_net(net_id)),
            Expr::UnOp(_loc, _typ, _op, e) => e.depends_on_net(net_id),
            Expr::BinOp(_loc, _typ, _op, e1, e2) => e1.depends_on_net(net_id) || e2.depends_on_net(net_id),
            Expr::If(_loc, _typ, cond, e1, e2) => cond.depends_on_net(net_id) || e1.depends_on_net(net_id) || e2.depends_on_net(net_id),
            Expr::Mux(_loc, _typ, cond, e1, e2) => cond.depends_on_net(net_id) || e1.depends_on_net(net_id) || e2.depends_on_net(net_id),
            Expr::Cat(_loc, _typ, es) => es.iter().any(|e| e.depends_on_net(net_id)),
            Expr::Sext(_loc, _typ, e) => e.depends_on_net(net_id),
            Expr::ToWord(_loc, _typ, e) => e.depends_on_net(net_id),
            Expr::Vec(_loc, _typ, es) => es.iter().any(|e| e.depends_on_net(net_id)),
            Expr::Idx(_loc, _typ, e, _i) => e.depends_on_net(net_id),
            Expr::IdxRange(_loc, _typ, e, _j, _i) => e.depends_on_net(net_id),
            //Expr::IdxDyn(_loc, e, i) => e.depends_on_net(net_id) || i.depends_on_net(net_id),
            Expr::Hole(_loc, _typ, _name) => false,
        }
    }

    pub fn rebase(&self, current_path: Path) -> Arc<Expr> {
        self.rebase_rec(current_path, &BTreeSet::new())
    }

    fn rebase_rec(&self, current_path: Path, shadowed: &BTreeSet<Path>) -> Arc<Expr> {
        Arc::new(match self {
            Expr::Reference(loc, typ, path) => {
                if !shadowed.contains(path) {
                    Expr::Reference(loc.clone(), typ.clone(), current_path.join(path.clone()))
                } else {
                    self.clone()
                }
            },
            Expr::Net(_loc, _typ, _net_id) => panic!("rebase() only works on reference expressions."),
            Expr::Word(_loc, _typ, _width, _value) => self.clone(),
            Expr::Enum(_loc, _typ, _typedef, _name) => self.clone(),
            Expr::Ctor(loc, typ, name, es) => Expr::Ctor(loc.clone(), typ.clone(), name.clone(), es.iter().map(|e| e.rebase_rec(current_path.clone(), shadowed)).collect()),
            Expr::Let(loc, typ, name, e, b) => {
                let new_e = e.rebase_rec(current_path.clone(), shadowed);
                let mut new_shadowed = shadowed.clone();
                new_shadowed.insert(name.clone().into());
                let new_b = b.rebase_rec(current_path, &new_shadowed);
                Expr::Let(loc.clone(), typ.clone(), name.clone(), new_e, new_b)
            },
            Expr::Match(_loc, _typ, _e, arms) => todo!(),
            Expr::UnOp(loc, typ, op, e) => Expr::UnOp(loc.clone(), typ.clone(), *op, e.rebase_rec(current_path, shadowed)),
            Expr::BinOp(loc, typ, op, e1, e2) => {
                Expr::BinOp(
                    loc.clone(),
                    typ.clone(),
                    *op,
                    e1.rebase_rec(current_path.clone(), shadowed),
                    e2.rebase_rec(current_path, shadowed),
                )
            },
            Expr::If(loc, typ, cond, e1, e2) => {
                Expr::If(
                    loc.clone(),
                    typ.clone(),
                    cond.rebase_rec(current_path.clone(), shadowed),
                    e1.rebase_rec(current_path.clone(), shadowed),
                    e2.rebase_rec(current_path, shadowed),
                )
            },
            Expr::Mux(loc, typ, cond, e1, e2) => {
                Expr::Mux(
                    loc.clone(),
                    typ.clone(),
                    cond.rebase_rec(current_path.clone(), shadowed),
                    e1.rebase_rec(current_path.clone(), shadowed),
                    e2.rebase_rec(current_path, shadowed),
                )
            },
            Expr::Cat(loc, typ, es) => {
                Expr::Cat(
                    loc.clone(),
                    typ.clone(),
                    es.iter().map(|e| e.rebase_rec(current_path.clone(), shadowed)).collect(),
                )
            },
            Expr::Sext(loc, typ, e) => Expr::Sext(loc.clone(), typ.clone(), e.rebase_rec(current_path, shadowed)),
            Expr::ToWord(loc, typ, e) => Expr::ToWord(loc.clone(), typ.clone(), e.rebase_rec(current_path, shadowed)),
            Expr::Vec(loc, typ, es) => Expr::Vec(loc.clone(), typ.clone(), es.iter().map(|e| e.rebase_rec(current_path.clone(), shadowed)).collect()),
            Expr::Idx(loc, typ, e, i) => Expr::Idx(loc.clone(), typ.clone(), e.rebase_rec(current_path, shadowed), *i),
            Expr::IdxRange(loc, typ, e, j, i) => Expr::IdxRange(loc.clone(), typ.clone(), e.rebase_rec(current_path, shadowed), *j, *i),
/*            Expr::IdxDyn(loc, e, i) => {
                Expr::IdxDyn(
                    loc.clone(),
                    e.rebase_rec(current_path.clone(), shadowed),
                    i.rebase_rec(current_path, shadowed),
                )
            }, */
            Expr::Hole(loc, typ, name) => Expr::Hole(loc.clone(), typ.clone(), name.clone()),
        })
    }

    /// Replace all references (see [`Expr::Reference`]) with nets (see [`Expr::Net`])
    /// to get it ready for simulation.
    pub fn references_to_nets(&self, net_id_by_path: &BTreeMap<Path, NetId>) -> Arc<Expr> {
        self.references_to_nets_rec(net_id_by_path, &BTreeSet::new())
    }

    fn references_to_nets_rec(&self, net_id_by_path: &BTreeMap<Path, NetId>, shadowed: &BTreeSet<Path>) -> Arc<Expr> {
        Arc::new(match self {
            Expr::Reference(loc, typ, path) => {
                if !shadowed.contains(path) {
                    Expr::Net(loc.clone(), typ.clone(), net_id_by_path[path])
                } else {
                    self.clone()
                }
            },
            Expr::Net(_loc, _typ, _net_id) => panic!("references_to_nets() only works on reference expressions."),
            Expr::Word(_loc, _typ, _width, _value) => self.clone(),
            Expr::Enum(_loc, _typ, _typedef, _name) => self.clone(),
            Expr::Ctor(loc, typ, name, es) => {
                Expr::Ctor(
                    loc.clone(),
                    typ.clone(),
                    name.clone(),
                    es.iter().map(|e| e.references_to_nets_rec(net_id_by_path, shadowed)).collect(),
                )

            },
            Expr::Let(loc, typ, name, e, b) => {
                let new_e = e.references_to_nets_rec(net_id_by_path, shadowed);
                let mut new_shadowed = shadowed.clone();
                new_shadowed.insert(name.clone().into());
                let new_b = b.references_to_nets_rec(net_id_by_path, &new_shadowed);
                Expr::Let(loc.clone(), typ.clone(), name.clone(), new_e, new_b)
            },
            Expr::UnOp(loc, typ, op, e) => Expr::UnOp(loc.clone(), typ.clone(), *op, e.references_to_nets_rec(net_id_by_path, shadowed)),
            Expr::BinOp(loc, typ, op, e1, e2) => {
                Expr::BinOp(
                    loc.clone(),
                    typ.clone(),
                    *op,
                    e1.references_to_nets_rec(net_id_by_path, shadowed),
                    e2.references_to_nets_rec(net_id_by_path, shadowed),
                )
            },
            Expr::If(loc, typ, cond, e1, e2) => {
                Expr::If(
                    loc.clone(),
                    typ.clone(),
                    cond.references_to_nets_rec(net_id_by_path, shadowed),
                    e1.references_to_nets_rec(net_id_by_path, shadowed),
                    e2.references_to_nets_rec(net_id_by_path, shadowed),
                )
            },
            Expr::Match(_loc, typ, _e, arms) => todo!(),
            Expr::Mux(loc, typ, cond, e1, e2) => {
                Expr::Mux(
                    loc.clone(),
                    typ.clone(),
                    cond.references_to_nets_rec(net_id_by_path, shadowed),
                    e1.references_to_nets_rec(net_id_by_path, shadowed),
                    e2.references_to_nets_rec(net_id_by_path, shadowed),
                )
            },
            Expr::Cat(loc, typ, es) => {
                Expr::Cat(
                    loc.clone(),
                    typ.clone(),
                    es.iter().map(|e| e.references_to_nets_rec(net_id_by_path, shadowed)).collect(),
                )
            },
            Expr::Sext(loc, typ, e) => Expr::Sext(loc.clone(), typ.clone(), e.references_to_nets_rec(net_id_by_path, shadowed)),
            Expr::ToWord(loc, typ, e) => Expr::ToWord(loc.clone(), typ.clone(), e.references_to_nets_rec(net_id_by_path, shadowed)),
            Expr::Vec(loc, typ, es) => Expr::Vec(loc.clone(), typ.clone(), es.iter().map(|e| e.references_to_nets_rec(net_id_by_path, shadowed)).collect()),
            Expr::Idx(loc, typ, e, i) => Expr::Idx(loc.clone(), typ.clone(), e.references_to_nets_rec(net_id_by_path, shadowed), *i),
            Expr::IdxRange(loc, typ, e, j, i) => Expr::IdxRange(loc.clone(), typ.clone(), e.references_to_nets_rec(net_id_by_path, shadowed), *j, *i),
/*            Expr::IdxDyn(loc, e, i) => {
                Expr::IdxDyn(
                    loc.clone(),
                    e.references_to_nets_rec(net_id_by_path, shadowed),
                    i.references_to_nets_rec(net_id_by_path, shadowed),
                )
            }, */
            Expr::Hole(loc, typ, name) => Expr::Hole(loc.clone(), typ.clone(), name.clone()),
        })
    }

    fn type_of(&self) -> Option<&OnceCell<Arc<Type>>> {
        match self {
            Expr::Net(_loc, typ, _netid) => Some(typ),
            Expr::Reference(_loc, typ, _path) => Some(typ),
            Expr::Word(_loc, typ, _width, _val) => Some(typ),
            Expr::Enum(_loc, typ, _typedef, _name) => Some(typ),
            Expr::Ctor(_loc, typ, _name, _e) => Some(typ),
            Expr::Let(_loc, typ, _name, _e, _b) => Some(typ),
            Expr::UnOp(_loc, typ, _op, _e) => Some(typ),
            Expr::BinOp(_loc, typ, _op, _e1, _e2) => Some(typ),
            Expr::If(_loc, typ, _cond, _e1, _e2) => Some(typ),
            Expr::Match(_loc, typ, _e, _arms) => Some(typ),
            Expr::Mux(_loc, typ, _cond, _e1, _e2) => Some(typ),
            Expr::Cat(_loc, typ, _es) => Some(typ),
            Expr::Sext(_loc, typ, _e) => Some(typ),
            Expr::ToWord(_loc, typ, _e) => Some(typ),
            Expr::Vec(_loc, typ, _es) => Some(typ),
            Expr::Idx(_loc, typ, _e, _i) => Some(typ),
            Expr::IdxRange(_loc, typ, _e, _j, _i) => Some(typ),
            //Expr::IdxDyn(_loc, typ, _e, _i) => Some(typ),
            Expr::Hole(_loc, typ, _opt_name) => Some(typ),
        }
    }
}
