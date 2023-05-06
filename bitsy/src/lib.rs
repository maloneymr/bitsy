#![allow(unused, dead_code)]

use std::collections::HashSet;
use ast::Namespace;
use parser::NamespaceParser;

use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Private,
}

#[macro_use]
extern crate lalrpop_util;

lalrpop_mod!(pub parser);

pub mod shapecheck;
pub mod ast;
pub mod depends;
// pub mod sim;
// pub mod nettle;

#[derive(Debug)]
pub struct Bitsy {
    modules: Vec<Arc<Module>>,
    shape_families: Vec<Arc<ShapeFamily>>,
}

impl Bitsy {
    pub fn new() -> Bitsy {
        Bitsy {
            modules: vec![],
            shape_families: vec![],
        }
    }

    pub fn add(&mut self, text: &str)  {
        let parser = NamespaceParser::new();
        let namespace = parser.parse(text).unwrap();
        //dbg!(&namespace);

        self.add_from(&namespace);
        //dbg!(&self);
    }

    fn add_from(&mut self, namespace: &Namespace) {
        let mut decl_names = HashSet::new();
        for decl in &namespace.decls {
            assert!(!decl_names.contains(decl.name()), "Duplicate declaration: {}", decl.name());
            decl_names.insert(decl.name().clone());
        }

        let mut depends = depends::Depends::<String>::new();

        for shape_def in &namespace.shape_defs() {
            depends.add(dbg!(shape_def.name().to_string()));
            for dep_shape_def in &shape_def.shape_refs() {
                depends.add_dependency(dep_shape_def.0.to_string(), shape_def.name().to_string());
            }
        }

        println!("Adding shape families");
        self.add_builtin_shape_families();
        for shape_ref in depends.sort().expect("Cycle detected") {
            println!("    {shape_ref}");
            // if not defined, define it
            if self.shape_family(&shape_ref).is_none() {
                self.add_shape_family(&namespace.shape_def(&shape_ref));
            }
        }

        println!();

        let mut depends = depends::Depends::<String>::new();

        for mod_def in &namespace.mod_defs() {
            depends.add(mod_def.name.clone());
            for dep_mod_def in &mod_def.depends_on() {
                depends.add_dependency(mod_def.name.clone(), dep_mod_def.to_string());
            }
        }

        for mod_name in depends.sort().expect("Cycle detected") {
            println!("    {mod_name}");
            self.add_module(&namespace.mod_def(&mod_name));
        }
    }

    fn add_builtin_shape_families(&mut self) {
        use ShapeParamType::{Nat, Shape};
        let builtins = [
            ShapeFamily { name: "Tuple".to_string(), args: None },
            ShapeFamily { name: "Bit".to_string(), args: Some(vec![]) },
            ShapeFamily { name: "Word".to_string(), args: Some(vec![Nat]) },
        ];
        for builtin in builtins {
            self.shape_families.push(Arc::new(builtin));
        }
    }

    fn add_shape_family(&mut self, shape_def: &ast::ShapeDef) {
        let args: Option<Vec<ShapeParamType>> = Some(shape_def.params().iter().map(|param| {
            match param {
                ast::ShapeDefParam::Nat(_name) => ShapeParamType::Nat,
                ast::ShapeDefParam::Shape(_name) => ShapeParamType::Shape,
            }
        }).collect());
        let shape_family = ShapeFamily {
            name: shape_def.name().to_string(),
            args,
        };
        self.shape_families.push(Arc::new(shape_family));
    }

    fn add_module(&mut self, mod_def: &ast::ModDef) {
        let module = Module {
            ports: vec![],
        };
        self.modules.push(Arc::new(module));
    }

    fn shape_family(&self, name: &str) -> Option<Arc<ShapeFamily>> {
        for shape_family in &self.shape_families {
            if shape_family.name == name {
                return Some(shape_family.clone());
            }
        }
        None
    }

    fn modules(&self) -> Vec<Arc<Module>> {
        todo!()
    }

    fn module(&self, name: &str) -> Option<Arc<Module>> {
        todo!()
    }
}

#[derive(Debug)]
pub struct Module {
    pub ports: Vec<Port>,
}

#[derive(Debug)]
pub struct ShapeFamily {
    pub name: String,
    pub args: Option<Vec<ShapeParamType>>, // Option for variadic
}

#[derive(Debug)]
pub enum ShapeParamType {
    Nat,
    Shape,
}


pub type ComponentName = String;
pub type PortName = String;
pub type FieldName = String;

#[derive(Debug)]
pub struct Port(PortName, Arc<Shape>);

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Shape {
    Bit,
    Word(u64),
    Tuple(Vec<Arc<Shape>>),
    Enum(Arc<EnumShape>),
    Struct(Arc<StructShape>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumShape {
    name: String,
    alts: Vec<EnumAlt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumAlt {
    ctor_name: String,
    visibility: Visibility,
    payload: Option<Arc<Shape>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructShape {
    name: String,
    visibility: Visibility,
    fields: Vec<StructField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructField(FieldName, Arc<Shape>);

impl Shape {
    pub fn bitwidth(&self) -> u64 {
        match self {
            Shape::Bit => 1,
            Shape::Word(n) => *n,
            Shape::Tuple(shapes) => shapes.iter().map(|shape| shape.bitwidth()).sum(),
            Shape::Enum(enum_shape) => todo!(), // enum_def.bitwidth(),
            Shape::Struct(struct_shape) => todo!(), // struct_def.bitwidth(),
        }
    }
}


/*
{
    fn flatten_exprs(&mut self) {
        let mut gensym_id = 0;
        let mut expr_wire_indexes: Vec<usize> = vec![];
        let mut wires_to_add: Vec<Wire> = vec![];
        let mut components_to_add: Vec<Component> = vec![];

        for (i, wire) in self.wires.iter().enumerate() {
            if let Wire::Expr(visibility, sink, expr) = wire {
                expr_wire_indexes.push(i);
                let (new_components, new_wires, source) = self.flatten_expr(
                    expr.clone(),
                    &mut gensym_id,
                );
                components_to_add.extend_from_slice(&new_components);
                wires_to_add.extend_from_slice(&new_wires);
                wires_to_add.push(Wire::Simple(*visibility, sink.clone(), source));
            }
        }

        for i in expr_wire_indexes.into_iter().rev() {
            self.wires.remove(i);
        }

        self.components.extend_from_slice(&components_to_add);
        self.wires.extend_from_slice(&wires_to_add);
    }

    fn flatten_expr(
        &self,
        expr: Arc<Expr>,
        gensym_id: &mut usize,
    ) -> (
        Vec<Component>,
        Vec<Wire>,
        Terminal,
    ) {
        match &*expr {
            Expr::Term(terminal) => {
                (
                    vec![],
                    vec![],
                    terminal.clone(),
                )
            },
            Expr::Lit(v) => {
                let name = format!("__gen_{gensym_id}");
                *gensym_id += 1;
                let gate = Component::Const(name, Visibility::Private, v.clone());
                (
                    vec![gate.clone()],
                    vec![],
                    gate.port("val"),
                )
            },
            Expr::Add(op0, op1) => {
                let (new_components0, new_wires0, op_terminal0) = self.flatten_expr(op0.clone(), gensym_id);
                let (new_components1, new_wires1, op_terminal1) = self.flatten_expr(op1.clone(), gensym_id);

                let name = format!("__gen_{gensym_id}");
                *gensym_id += 1;
                let gate = Component::Gate(name, Visibility::Private, GateComponent { gate_name: "Add".to_string() });
                (
                    vec![gate.clone()],
                    vec![
                        Wire::Simple(Visibility::Private, gate.port("in0"), op_terminal0),
                        Wire::Simple(Visibility::Private, gate.port("in1"), op_terminal1),
                    ],
                    gate.port("out"),
                )
            },
            Expr::Mul(op0, op1) => {
                let (new_components0, new_wires0, op_terminal0) = self.flatten_expr(op0.clone(), gensym_id);
                let (new_components1, new_wires1, op_terminal1) = self.flatten_expr(op1.clone(), gensym_id);

                let name = format!("__gen_{gensym_id}");
                *gensym_id += 1;
                let gate = Component::Gate(name, Visibility::Private, GateComponent { gate_name: "Mul".to_string() });
                (
                    vec![gate.clone()],
                    vec![
                        Wire::Simple(Visibility::Private, gate.port("in0"), op_terminal0),
                        Wire::Simple(Visibility::Private, gate.port("in1"), op_terminal1),
                    ],
                    gate.port("out"),
                )
            },
        }
    }
}
impl Bitsy {
    pub fn flatten_exprs(&self) -> Bitsy {
        let mut decls = vec![];

        for decl in &self.decls {
            match decl {
                Decl::ModDef(mod_def) => {
                    let mut flattened_mod_def: ModDef = ModDef::clone(mod_def);
                    flattened_mod_def.flatten_exprs();
                    decls.push(Decl::ModDef(Arc::new(flattened_mod_def)));
                },
                Decl::EnumDef(enum_def) => {
                    decls.push(Decl::EnumDef(Arc::clone(enum_def)));
                },
                Decl::StructDef(struct_def) => {
                    decls.push(Decl::StructDef(Arc::clone(struct_def)));
                },
            }
        }
        Bitsy {
            decls,
        }
    }
}
*/

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Value {
    Unknown,
    Unobservable,
    Bit(bool),
    Word(u64),
    Tuple(Vec<Box<Value>>),
}

impl Value {
    pub fn from(value: ast::Value) -> Value {
        match value {
            ast::Value::Bit(b) => Value::Bit(b),
            ast::Value::Word(n) => Value::Word(n),
            ast::Value::Tuple(vs) => {
                let mut new_vs = vec![];
                for v in vs {
                    new_vs.push(Box::new(Value::from(*v.clone())));
                }

                Value::Tuple(new_vs)
            },
            _ => panic!("Uh oh"),
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Value::Unknown => write!(f, "?")?,
            Value::Unobservable => write!(f, "X")?,
            Value::Bit(b) => write!(f,"{b}")?,
            Value::Word(n) => write!(f, "{n}")?,
            Value::Tuple(elts) => {
                write!(f, "tuple(")?;
                for (i, elt) in elts.iter().enumerate() {
                    write!(f, "{elt}")?;
                    if i + 1 < elts.len() {
                        write!(f, ", ")?;
                    }
                }
                write!(f, "tuple)")?;
            },
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum Expr {
    Term(Terminal),
    Lit(Value),
    Let(String, Box<Expr>, Option<Shape>, Box<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    As(Box<Expr>, Arc<Shape>),
}

impl Expr {
    pub fn from(expr: &ast::Expr) -> Box<Expr> {
        Box::new(match expr {
            ast::Expr::Term(ast::Terminal(component, port)) => Expr::Term(Terminal(component.to_string(), port.to_string())),
            ast::Expr::Lit(v) => Expr::Lit(Value::from(v.clone())),
            ast::Expr::Let(x, def, def_shape, body) => Expr::Let(x.to_string(), Expr::from(def), todo!(), Expr::from(body)),
            ast::Expr::Add(op0, op1) => Expr::Add(Expr::from(op0), Expr::from(op1)),
            ast::Expr::Mul(op0, op1) => Expr::Mul(Expr::from(op0), Expr::from(op1)),
            //ast::Expr::As(Box<Expr>, Arc<Shape>),
            _ => panic!("No as"),
        })
    }
}

#[derive(Debug, Clone)]
pub struct Terminal(pub ComponentName, pub PortName);

impl Terminal {
    pub fn component(&self) -> &ComponentName {
        &self.0
    }

    pub fn port(&self) -> &PortName {
        &self.1
    }
}

// impl EnumDef {
//     fn clog2(n: u64) -> u64 {
//         let mut r = 0;
//         while (1 << r) > n {
//             r += 1;
//         }
//         r
//     }
// }
//    pub fn to_shape(&self, shape_ref: &ShapeRef) -> Arc<Shape> {
//        let ShapeRef(shape_name, params) = shape_ref;
//        if shape_name == &"Bit" {
//            assert_eq!(params.len(), 0, "Bit doesn't take params");
//            return Arc::new(Shape::Bit);
//        } else if shape_name == &"Word" {
//            assert_eq!(params.len(), 1, "Word takes exactly 1 param");
//            if let ShapeParam::Nat(n) = params[0] {
//                return Arc::new(Shape::Word(n));
//            } else {
//                panic!("Word takes exactly 1 Nat param");
//            }
//        }
//        panic!("Unknown shape: {shape_ref:?}")
//    }

