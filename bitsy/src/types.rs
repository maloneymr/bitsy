use super::Context;
use super::Path;
use super::Expr;
use crate::reference::Reference;
use std::sync::Arc;

/// The bitwidth of a [`Type::Word`].
pub type Width = u64;

/// The length of a [`Type::Vec`].
pub type Length = u64;

/// A type classifier for [`Value`]s.
#[derive(Clone, PartialEq)]
pub enum Type {
    /// An n-bit two's complement integer. Nominally unsigned. Written `Word<n>`.
    Word(Width),
    /// A n-element vector. Written `Vec<T, n>`.
    Vec(Arc<Type>, Length),
    /// An optional value. Written `Valid<T>`.
    Valid(Arc<Type>),
    /// A user-defined `enum`.
    Enum(Arc<EnumTypeDef>),
    /// A user-defined `struct`.
    Struct(Arc<StructTypeDef>),
    /// An unresolved reference to a user-defined type.
    TypeRef(Reference<Type>),
}

impl Type {
    pub fn word(w: Width) -> Arc<Type> {
        Arc::new(Type::Word(w))
    }

    pub fn vec(typ: Arc<Type>, n: Length) -> Arc<Type> {
        Arc::new(Type::Vec(typ, n))
    }

    pub fn bitwidth(&self) -> Width {
        match self {
            Type::Word(n) => *n,
            Type::Valid(typ) => typ.bitwidth() + 1,
            Type::Vec(typ, n) => typ.bitwidth() * n,
            Type::Enum(typedef) => typedef.bitwidth(),
            Type::Struct(typedef) => typedef.bitwidth(),
            Type::TypeRef(typeref) => typeref.get().unwrap().bitwidth(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordLit(pub Option<Width>, pub u64);

/// A user-defined `enum` type.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumTypeDef {
    pub name: String,
    pub values: Vec<(String, WordLit)>,
}

/// A user-defined `struct` type.
#[derive(Debug, Clone, PartialEq)]
pub struct StructTypeDef {
    pub name: String,
    pub fields: Vec<(String, Arc<Type>)>,
}

/// A user-defined `fn` function.
#[derive(Debug, Clone)]
pub struct FnDef {
    pub name: String,
    pub args: Vec<(String, Arc<Type>)>,
    pub ret: Arc<Type>,
    pub body: Arc<Expr>,
}

impl FnDef {
    pub fn context(&self) -> Context<Path, Arc<Type>> {
        Context::from(self.args.iter().map(|(arg_name, arg_type)| (arg_name.to_string().into(), arg_type.clone())).collect::<Vec<_>>())
    }
}

impl StructTypeDef {
    fn bitwidth(&self) -> Width {
        self.fields.iter().map(|(_name, typ)| typ.bitwidth()).sum()
    }
}

impl EnumTypeDef {
    pub fn value_of(&self, name: &str) -> Option<u64> {
        for (other_name, WordLit(_w, value)) in &self.values {
            if name == other_name {
                return Some(*value);
            }
        }
        None
    }

    pub fn bitwidth(&self) -> Width {
        // TODO
        let mut max_width = None;
        for (_name, value) in &self.values {
            if let WordLit(Some(w), _n) = value {
                if let Some(max_w) = max_width {
                    assert_eq!(*w, max_w);
                } else {
                    max_width = Some(*w);
                }
             }
        }
        // TODO
        max_width.unwrap()
    }
}

impl std::fmt::Debug for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Type::Word(n) => write!(f, "Word<{n}>"),
            Type::Valid(typ) => write!(f, "Valid<{typ:?}>"),
            Type::Vec(typ, n) => write!(f, "Vec<{typ:?}, {n}>"),
            Type::Struct(typedef) => write!(f, "{}", typedef.name),
            Type::Enum(typedef) => write!(f, "{}", typedef.name),
            Type::TypeRef(reference) => write!(f, "{}", reference.name()),
        }
    }
}
