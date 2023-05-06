pub type ComponentName = String;
pub type PortName = String;

#[derive(Debug)]
pub struct Namespace {
    pub decls: Vec<Decl>,
}

impl Namespace {
    pub fn mod_defs(&self) -> Vec<&ModDef> {
        let mut result = vec![];
        for decl in &self.decls {
            if let Decl::ModDef(mod_def) = decl {
                result.push(mod_def);
            }
        }
        result
    }

    pub fn enum_defs(&self) -> Vec<&EnumDef> {
        let mut result = vec![];
        for decl in &self.decls {
            if let Decl::EnumDef(enum_def) = decl {
                result.push(enum_def);
            }
        }
        result
    }

    pub fn mod_def(&self, name: &str) -> &ModDef {
        for mod_def in self.mod_defs() {
            if mod_def.name == name {
                return &mod_def;
            }
        }

        panic!("No such module found: {name}")
    }

    pub fn enum_def(&self, name: &str) -> &EnumDef {
        for enum_def in self.enum_defs() {
            if enum_def.name == name {
                return &enum_def;
            }
        }

        panic!("No such enum found: {name}")
    }

    pub fn shape_defs(&self) -> Vec<ShapeDef> {
        let mut results = vec![];
        for decl in &self.decls {
            if let Decl::EnumDef(enum_def) = decl {
                results.push(ShapeDef::EnumDef(enum_def.clone()));
            } else if let Decl::StructDef(struct_def) = decl {
                results.push(ShapeDef::StructDef(struct_def.clone()));
            }
        }
        results
    }

    pub fn shape_def(&self, name: &str) -> ShapeDef {
        for decl in &self.decls {
            if decl.name() == name {
                if let Decl::EnumDef(enum_def) = decl {
                    return ShapeDef::EnumDef(enum_def.clone());
                } else if let Decl::StructDef(struct_def) = decl {
                    return ShapeDef::StructDef(struct_def.clone());
                } else {
                    panic!("Decl {name} is not a shape decl")
                }
            }
        }

        panic!("No such shape def found: {name}")
    }
}

#[derive(Debug)]
pub enum ShapeDef {
    EnumDef(EnumDef),
    StructDef(StructDef),
}

impl ShapeDef {
    pub fn shape_refs(&self) -> Vec<ShapeRef> {
        match self {
            ShapeDef::EnumDef(enum_def) => enum_def.shape_refs(),
            ShapeDef::StructDef(struct_def) => struct_def.shape_refs(),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            ShapeDef::EnumDef(enum_def) => &enum_def.name,
            ShapeDef::StructDef(struct_def) => &struct_def.name,
        }
    }

    pub fn params(&self) -> Vec<ShapeDefParam> {
        match self {
            ShapeDef::EnumDef(enum_def) => enum_def.params.clone(),
            ShapeDef::StructDef(struct_def) => struct_def.params.clone(),
        }
    }
}

#[derive(Debug)]
pub enum Decl {
    ModDef(ModDef),
    EnumDef(EnumDef),
    StructDef(StructDef),
}

impl Decl {
    pub fn name(&self) -> &str {
        match self {
            Decl::ModDef(mod_def) => &mod_def.name,
            Decl::EnumDef(enum_def) => &enum_def.name,
            Decl::StructDef(struct_def) => &struct_def.name,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: String,
    pub params: Vec<ShapeDefParam>,
    pub visibility: Visibility,
    pub fields: Vec<StructField>,
}

impl StructDef {
    pub fn shape_refs(&self) -> Vec<ShapeRef> {
        let mut results = vec![];
        for StructField(_field_name, shape_ref) in &self.fields {
            results.push(shape_ref.clone());
        }
        results
    }
}

pub type FieldName = String;

#[derive(Debug, Clone)]
pub struct StructField(pub FieldName, pub ShapeRef);

#[derive(Debug, Clone)]
pub struct EnumDef {
    pub name: String,
    pub params: Vec<ShapeDefParam>,
    pub visibility: Visibility,
    pub alts: Vec<EnumAlt>,
}

impl EnumDef {
    pub fn shape_refs(&self) -> Vec<ShapeRef> {
        let mut results = vec![];
        for alt in &self.alts {
            if let Some(shape_ref) = &alt.payload_shape {
                results.push(shape_ref.clone());
            }
        }
        results
    }
}

#[derive(Debug, Clone)]
pub struct EnumAlt {
    pub ctor_name: String,
    pub payload_shape: Option<ShapeRef>,
}

#[derive(Debug, Clone)]
pub struct ModDef {
    pub name: String,
    pub visibility: Visibility,
    pub ports: Vec<Port>,
    pub components: Vec<Component>,
    pub wires: Vec<Wire>,
}

impl ModDef {
    pub fn depends_on(&self) -> Vec<ModDefRef> {
        let mut result = vec![];
        for component in &self.components {
            if let Component::Mod(_name, _visibility, mod_component) = component {
                result.push(mod_component.mod_def_ref.clone());
            }
        }
        result
    }
}

#[derive(Debug, Clone)]
pub enum Wire {
    Simple(Visibility, TerminalRef, TerminalRef),
    Expr(Visibility, TerminalRef, Box<Expr>),
}

#[derive(Debug, Clone)]
pub enum Expr {
    Var(String),
    Lit(Value),
    Let(String, Box<Expr>, Option<ShapeRef>, Box<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    As(Box<Expr>, ShapeRef),
    Match(Box<Expr>, Vec<MatchArm>),
}

#[derive(Debug, Clone)]
pub struct MatchArm(pub Box<MatchPattern>, pub Box<Expr>);

#[derive(Debug, Clone)]
pub enum MatchPattern {
    Ctor(String, Vec<Box<MatchPattern>>),
    Var(String),
    Lit(Value),
    Otherwise,
}

impl Wire {
    pub fn visibility(&self) -> Visibility {
        match self {
            Wire::Simple(visibility, _sink, _source) => *visibility,
            Wire::Expr(visibility, _sink, _expr) => *visibility,
        }
    }

    pub fn sink(&self) -> &TerminalRef {
        match self {
            Wire::Simple(_visibility, sink, _source) => sink,
            Wire::Expr(_visibility, sink, _expr) => sink,
        }
    }

    /*
    pub fn source(&self) -> &TerminalRef {
        &self.2
    }
    */
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Direction {
    Incoming,
    Outgoing,
}

#[derive(Debug, Clone, Copy)]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Debug, Clone)]
pub enum Component {
    Reg(ComponentName, Visibility, RegComponent),
    Mod(ComponentName, Visibility, ModComponent),
    Gate(ComponentName, Visibility, GateComponent),
    Const(ComponentName, Visibility, Value),
}

impl Component {
    pub fn name(&self) -> &str {
        match self {
            Component::Reg(name, _, _) => name,
            Component::Mod(name, _, _) => name,
            Component::Gate(name, _, _) => name,
            Component::Const(name, _, _) => name,
        }
    }

    pub fn port<T>(&self, port_name: T) -> TerminalRef
        where T: Into<PortName> {
        TerminalRef(self.name().to_string(), port_name.into())
    }
}

#[derive(Debug, Clone)]
pub struct RegComponent {
    pub shape: ShapeRef,
    pub domain: DomainRef,
    pub init: Value,
}

#[derive(Debug, Clone)]
pub struct ModComponent {
    pub mod_def_ref: ModDefRef,
}

#[derive(Debug, Clone)]
pub struct GateComponent {
    pub gate_ref: GateRef,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Value {
    Unknown,
    Unobservable,
    Bit(bool),
    Word(u64),
    Tuple(Vec<Box<Value>>),
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
pub struct Port(pub String, pub Direction, pub ShapeRef, pub DomainRef);

impl Port {
    pub fn name(&self) -> &str {
        &self.0
    }

    pub fn direction(&self) -> Direction {
        self.1
    }

    pub fn shape(&self) -> &ShapeRef {
        &self.2
    }

    pub fn domain(&self) -> &DomainRef {
        &self.3
    }
}

#[derive(Debug, Clone)]
pub enum ShapeDefParam {
    Nat(String),
    Shape(String),
}

/*
impl ShapeRef {
    pub fn shape_refs(&self) -> Vec<ShapeRef> {
        let mut results = vec![self.0];
        for shape_param in &self.1 {
            if let ShapeParam::Shape(shape_ref) = shape_param {
                results.extend_from_slice(&shape_ref.shape_refs());
            }
        }
        results
    }
}
*/

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum ShapeParam {
    Nat(u64),
    Shape(ShapeRef),
}

#[derive(Eq, PartialEq, Clone, Debug)]
pub struct ShapeRef(pub String, pub Vec<ShapeParam>);
#[derive(Eq, PartialEq, Clone, Hash, PartialOrd, Ord, Debug)]
pub struct TerminalRef(pub String, pub String);
#[derive(Eq, PartialEq, Clone, Hash, PartialOrd, Ord, Debug)]
pub struct DomainRef(pub String);
#[derive(Eq, PartialEq, Clone, Hash, PartialOrd, Ord, Debug)]
pub struct ModDefRef(pub String);
#[derive(Eq, PartialEq, Clone, Hash, PartialOrd, Ord, Debug)]
pub struct GateRef(pub String);

impl std::fmt::Display for ModDefRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl std::fmt::Display for TerminalRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)?;
        std::fmt::Display::fmt(&self.1, f)
    }
}

impl std::fmt::Display for DomainRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl std::fmt::Display for GateRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl TerminalRef {
    pub fn component(&self) -> &ComponentName {
        &self.0
    }

    pub fn port(&self) -> &PortName {
        &self.1
    }
}
