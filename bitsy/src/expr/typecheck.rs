use super::*;
use crate::types::*;

impl Expr {
    #[allow(unused_variables)] // TODO remove this
    pub fn typecheck(self: &Arc<Self>, type_expected: Arc<Type>, ctx: Context<Path, Arc<Type>>) -> Result<(), TypeError> {
        if let Some(type_actual) = self.typeinfer(ctx.clone()) {
            if type_actual == type_expected {
                return Ok(());
            } else {
                return Err(TypeError::NotExpectedType(type_expected.clone(), type_actual.clone(), self.clone()));
            }
        }

        let result = match (&*type_expected.clone(), &**self) {
            (_type_expected, Expr::Reference(_loc, _typ, path)) => Err(TypeError::UndefinedReference(self.clone())),
            (Type::Word(width_expected), Expr::Word(_loc, typ, width_actual, n)) => {
                if let Some(width_actual) = width_actual {
                    if *width_actual == *width_expected {
                        Err(TypeError::Other(self.clone(), format!("Not the expected width")))
                    } else if n >> *width_actual != 0 {
                        Err(TypeError::Other(self.clone(), format!("Doesn't fit")))
                    } else {
                        Ok(())
                    }
                } else {
                    if n >> *width_expected != 0 {
                        Err(TypeError::Other(self.clone(), format!("Doesn't fit")))
                    } else {
                        Ok(())
                    }
                }
            },
            (Type::TypeDef(typedef_expected), Expr::Enum(_loc, typedef, _name)) => {
                if typedef_expected == typedef {
                    Ok(())
                } else {
                    Err(TypeError::Other(self.clone(), format!("Type Error")))
                }
            },
            (_type_expected, Expr::Ctor(loc, name, es)) => {
                // TODO
                if let Type::Valid(typ) = &*type_expected {
                    if es.len() == 1 {
                        es[0].typecheck(typ.clone(), ctx.clone())
                    } else if es.len() > 1 {
                        Err(TypeError::Other(self.clone(), format!("Error")))
                    } else {
                        Ok(())
                    }
                } else {
                    Err(TypeError::Other(self.clone(), format!("Not a Valid<T>: {self:?} is not {type_expected:?}")))
                }
            },
            (_type_expected, Expr::Let(_loc, name, e, b)) => {
                if let Some(typ) = e.typeinfer(ctx.clone()) {
                    b.typecheck(type_expected.clone(), ctx.extend(name.clone().into(), typ))
                } else {
                    Err(TypeError::Other(self.clone(), format!("Can infer type of {e:?} in let expression.")))
                }
            },
            (_type_expected, Expr::Match(_loc, _e, arms)) => Err(TypeError::Other(self.clone(), format!("match expressions are not yet implemented"))),
            (_type_expected, Expr::UnOp(_loc, UnOp::Not, e)) => e.typecheck(type_expected.clone(), ctx.clone()),
            (Type::Word(1), Expr::BinOp(_loc, BinOp::Eq | BinOp::Neq | BinOp::Lt, e1, e2)) => {
                if let Some(typ1) = e1.typeinfer(ctx.clone()) {
                    e2.typecheck(typ1, ctx.clone())?;
                    Ok(())
                } else {
                    Err(TypeError::Other(self.clone(), format!("Can't infer type.")))
                }
            },
            (Type::Word(n), Expr::BinOp(_loc, BinOp::Add | BinOp::Sub | BinOp::And | BinOp::Or | BinOp::Xor, e1, e2)) => {
                e1.typecheck(type_expected.clone(), ctx.clone())?;
                e2.typecheck(type_expected.clone(), ctx.clone())?;
                Ok(())
            },
            (Type::Word(n), Expr::BinOp(_loc, BinOp::AddCarry, e1, e2)) => {
                if let (Some(typ1), Some(typ2)) = (e1.typeinfer(ctx.clone()), e2.typeinfer(ctx.clone())) {
                    if *n > 0 && typ1 == typ2 && *typ1 == Type::Word(*n - 1) {
                        Ok(())
                    } else {
                        Err(TypeError::Other(self.clone(), format!("Types don't match")))
                    }
                } else {
                    Err(TypeError::Other(self.clone(), format!("Can't infer type.")))
                }
            },
            (_type_expected, Expr::If(_loc, cond, e1, e2)) => {
                cond.typecheck(Type::word(1), ctx.clone())?;
                e1.typecheck(type_expected.clone(), ctx.clone())?;
                e2.typecheck(type_expected.clone(), ctx.clone())?;
                Ok(())
            },
            (_type_expected, Expr::Mux(_loc, cond, e1, e2)) => {
                cond.typecheck(Type::word(1), ctx.clone())?;
                e1.typecheck(type_expected.clone(), ctx.clone())?;
                e2.typecheck(type_expected.clone(), ctx.clone())?;
                Ok(())
            },
            (Type::Word(width_expected), Expr::Sext(_loc, e, n)) => {
                if *n != *width_expected {
                    Err(TypeError::Other(self.clone(), format!("Type mismatch")))
                } else if let Some(type_actual) = e.typeinfer(ctx.clone()) {
                    if let Type::Word(m) = &*type_actual {
                        if n >= m {
                            Ok(())
                        } else {
                            Err(TypeError::Other(self.clone(), format!("Can't sext a Word<{m}> to a a Word<{n}>")))
                        }
                    } else {
                        Err(TypeError::Other(self.clone(), format!("Unknown?")))
                    }
                } else {
                    Err(TypeError::CantInferType(self.clone()))
                }
            },
            (Type::Word(_n), Expr::ToWord(_loc, e)) => {
                Err(TypeError::Other(self.clone(), format!("Not yet implemented.")))
            },
            (Type::Vec(typ, n), Expr::Vec(_loc, es)) => {
                for e in es {
                    e.typecheck(typ.clone(), ctx.clone())?;
                }
                if es.len() != *n as usize {
                    let type_actual = Type::vec(typ.clone(), es.len().try_into().unwrap());
                    Err(TypeError::NotExpectedType(type_expected.clone(), type_actual.clone(), self.clone()))
                } else {
                    Ok(())
                }
            },
            (_type_expected, Expr::Idx(_loc, e, i)) => {
                match e.typeinfer(ctx.clone()).as_ref().map(|arc| &**arc) {
                    Some(Type::Word(n)) if i < n => Ok(()),
                    Some(Type::Word(n)) => Err(TypeError::Other(self.clone(), format!("Index out of bounds"))),
                    Some(typ) => Err(TypeError::Other(self.clone(), format!("Can't index into type {typ:?}"))),
                    None => Err(TypeError::Other(self.clone(), format!("Can't infer the type of {e:?}"))),
                }
            },
            (_type_expected, Expr::IdxRange(_loc, e, j, i)) => {
                match e.typeinfer(ctx.clone()).as_ref().map(|arc| &**arc) {
                    Some(Type::Word(n)) if n >= j && j >= i => Ok(()),
                    Some(Type::Word(_n)) => Err(TypeError::Other(self.clone(), format!("Index out of bounds"))),
                    Some(typ) => Err(TypeError::Other(self.clone(), format!("Can't index into type {typ:?}"))),
                    None => Err(TypeError::Other(self.clone(), format!("Can't infer the type of {e:?}"))),
                }
            },
//            (_type_expected, Expr::IdxDyn(_loc, e, i)) => todo!(),
            (_type_expected, Expr::Hole(_loc, opt_name)) => Ok(()),
            _ => Err(TypeError::Other(self.clone(), format!("{self:?} is not the expected type {type_expected:?}"))),
        };

        if let Some(typ) = self.type_of() {
            if let Ok(()) = &result {
                let _ = typ.set(type_expected);
            }
        }
        result
    }

    #[allow(unused_variables)] // TODO remove this
    pub fn typeinfer(self: &Arc<Self>, ctx: Context<Path, Arc<Type>>) -> Option<Arc<Type>> {
        let result = match &**self {
            Expr::Reference(_loc, typ, path) => {
                let type_actual = ctx.lookup(path)?;
                Some(type_actual)
            },
            Expr::Net(_loc, _typ, netid) => panic!("Can't typecheck a net"),
            Expr::Word(_loc, _typ, None, n) => None,
            Expr::Word(_loc, _typ, Some(w), n) => if n >> w == 0 {
                Some(Type::word(*w))
            } else {
                None
            },
            Expr::Enum(_loc, typedef, _name) => Some(Arc::new(Type::TypeDef(typedef.clone()))),
            Expr::Cat(_loc, es) => {
                let mut w = 0u64;
                for e in es {
                    if let Some(Type::Word(m)) = e.typeinfer(ctx.clone()).as_ref().map(|arc| &**arc) {
                        w += m;
                    } else {
                        return None;
                    }
                }
                Some(Type::word(w))
            },
            Expr::ToWord(loc, e) => {
                match e.typeinfer(ctx.clone()).as_ref().map(|arc| &**arc) {
                    Some(Type::TypeDef(typedef)) => {
                        if let Some(typedef) = typedef.get() {
                            Some(Type::word(typedef.width()))
                        } else {
                            panic!("Unresolved typedef: {:?} location: {loc:?}", typedef.name())
                        }
                    }
                    _ => None,
                }
            },
            Expr::Vec(_loc, es) => None,
            Expr::Idx(_loc, e, i) => {
                match e.typeinfer(ctx.clone()).as_ref().map(|arc| &**arc) {
                    Some(Type::Word(n)) if i < n => Some(Type::word(1)),
                    _ => None,
                }
            },
            Expr::IdxRange(_loc, e, j, i) => {
                match e.typeinfer(ctx.clone()).as_ref().map(|arc| &**arc) {
                    Some(Type::Word(n)) if n >= j && j >= i => Some(Type::word(*j - *i)),
                    Some(Type::Word(n)) => None,
                    Some(typ) => None,
                    None => None,
                }
            },
            Expr::IdxDyn(_loc, e, i) => None,
            Expr::Hole(_loc, opt_name) => None,
            _ => None,
        };

        if let Some(type_actual) = &result {
            if let Some(typ) = self.type_of() {
                let _ = typ.set(type_actual.clone());
            }
        }
        result
    }
}
