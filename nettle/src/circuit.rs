use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Component {
    Incoming(Type),
    Outgoing(Type),
    Node(Type),
    Reg(Type, Value),
    Mod,
    Ext,
}

#[derive(Debug, Clone)]
pub struct Circuit(Arc<CircuitNode>);

#[derive(Debug)]
pub(crate) struct CircuitNode {
    paths: BTreeMap<Path, PathType>,
    components: BTreeMap<Path, Component>,
    wires: BTreeMap<Path, Expr>,
    path: Vec<String>,
    exts: Vec<Path>,
}

impl Circuit {
    pub fn new(name: &str) -> CircuitNode {
        let components = vec![("top".into(), Component::Mod)].into_iter().collect();
        CircuitNode {
            paths: BTreeMap::new(),
            components,
            wires: BTreeMap::new(),
            path: vec![name.to_string()],
            exts: vec![],
        }
    }

    pub fn components(&self) -> &BTreeMap<Path, Component> {
        &self.0.components
    }

    pub fn paths(&self) -> &BTreeMap<Path, PathType> {
        &self.0.paths
    }

    pub fn wires(&self) -> &BTreeMap<Path, Expr> {
        &self.0.wires
    }

    pub fn exts(&self) -> &[Path] {
        &self.0.exts
    }

    pub fn terminals(&self) -> Vec<Path> {
        let mut terminals = vec![];
        for (path, component) in self.components() {
            match component {
                Component::Incoming(_typ) => terminals.push(format!("{path}.val").into()),
                Component::Outgoing(_typ) => terminals.push(format!("{path}.set").into()),
                Component::Node(_typ) => {
                    terminals.push(format!("{path}.val").into());
                    terminals.push(format!("{path}.set").into());
                },
                Component::Reg(_typ, _reset) => {
                    terminals.push(format!("{path}.val").into());
                    terminals.push(format!("{path}.set").into());
                },
                Component::Mod => (),
                Component::Ext => (),
            }
        }
        terminals
    }

    pub fn nets(&self) -> Vec<Net> {
        let mut immediate_driver_for: BTreeMap<Path, Path> = BTreeMap::new();
        for (target, expr) in self.wires() {
            if let Expr::Reference(driver) = expr {
                immediate_driver_for.insert(target.clone(), driver.clone());
            }
        }
        dbg!(&self);
        for terminal in self.terminals() {
            println!("TERMINAL: {terminal}");
        }

        let mut drivers: BTreeSet<Path> = BTreeSet::new();
        for terminal in self.terminals() {
            drivers.insert(driver_for(terminal, &immediate_driver_for));
        }

        let mut nets: BTreeMap<Path, Net> = BTreeMap::new();
        for driver in &drivers {
            nets.insert(driver.clone(), Net::from(driver.clone()));
        }

        for terminal in self.terminals() {
            let driver = driver_for(terminal.clone(), &immediate_driver_for);
            let net = nets.get_mut(&driver).unwrap();
            net.add(terminal);
        }

        let nets: Vec<Net> = nets.values().into_iter().cloned().collect();
        nets
    }
}

fn driver_for(terminal: Path, immediate_driver_for: &BTreeMap<Path, Path>) -> Path {
    let mut driver: &Path = &terminal;
    while let Some(immediate_driver) = &immediate_driver_for.get(driver) {
        driver = immediate_driver;
    }
    driver.clone()
}

impl CircuitNode {
    fn push(mut self, path: &str) -> Self {
        self.path.push(path.to_string());
        self
    }

    fn pop(mut self) -> Self {
        self.path.pop();
        self
    }

    fn local_name_to_path(&self, name: &str) -> Path {
        let path = self.path.join(".");
        format!("{path}.{name}").into()
    }

    pub(crate) fn node(mut self, name: &str, typ: Type) -> Self {
        let path = self.local_name_to_path(name);
        self.paths.insert(path.clone(), PathType::Node(typ));
        self.components.insert(path.clone(), Component::Node(typ));
        let val_path: Path = format!("{}.val", self.local_name_to_path(name)).into();
        let set_path: Path = format!("{}.set", self.local_name_to_path(name)).into();
        self.wires.insert(val_path, Expr::Reference(set_path));
        self
    }

    pub(crate) fn incoming(mut self, name: &str, typ: Type) -> Self {
        let path = self.local_name_to_path(name);
        let set_path: Path = format!("{}.set", self.local_name_to_path(name)).into();
        let val_path: Path = format!("{}.val", self.local_name_to_path(name)).into();
        self.paths.insert(set_path, PathType::Outgoing(typ));
        self.paths.insert(val_path, PathType::Outgoing(typ));
        self.paths.insert(path.clone(), PathType::Incoming(typ));
        self.components.insert(path, Component::Incoming(typ));
        self
    }

    pub(crate) fn outgoing(mut self, name: &str, typ: Type) -> Self {
        let path = self.local_name_to_path(name);
        let set_path: Path = format!("{}.set", self.local_name_to_path(name)).into();
        let val_path: Path = format!("{}.val", self.local_name_to_path(name)).into();
        self.paths.insert(set_path, PathType::Outgoing(typ));
        self.paths.insert(val_path, PathType::Outgoing(typ));
        self.paths.insert(path.clone(), PathType::Outgoing(typ));
        self.components.insert(path, Component::Outgoing(typ));
        self
    }

    pub(crate) fn reg(mut self, name: &str, typ: Type, reset: Value) -> Self {
        let path = self.local_name_to_path(name);
        let set_path = format!("{path}.set");
        let val_path = format!("{path}.val");

        self.paths.insert(path.clone(), PathType::Reg(typ, reset));
        self.paths.insert(set_path.into(), PathType::Node(typ));
        self.paths.insert(val_path.into(), PathType::Node(typ));
        self.components.insert(path, Component::Reg(typ, reset));
        self
    }

    pub(crate) fn wire(mut self, name: &str, expr: &Expr) -> Self {
        let set_path: Path = format!("{}.set", self.local_name_to_path(name)).into();
        self.wires.insert(set_path, expr.clone().expand_references_as_val().relative_to(&self.current_path()));
        self
    }

    pub fn instantiate(mut self, name: &str, circuit: &CircuitNode) -> Self {
        let mod_path = self.current_path();
        self = self.push(name);
        self.components.insert(self.current_path(), Component::Mod);

        for (path, typ) in &circuit.paths {
            let target = relative_to(&mod_path, path);
            self.paths.insert(target, typ.clone());
        }

        for (path, component) in &circuit.components {
            if path != &"top".into() {
                let target = relative_to(&mod_path, path);
                self.components.insert(target, component.clone());
            }
        }

        for (path, expr) in &circuit.wires {
            let target = relative_to(&mod_path, path);
            let expr = expr.clone().relative_to(&mod_path);
            self.wires.insert(target, expr);
        }
        self = self.pop();
        self
    }

    fn current_path(&self) -> Path {
        self.path.join(".").into()
    }

    pub(crate) fn ext(mut self, name: &str, ports: &[(String, PortDirection, Type)]) -> Self {
        let ext = self.local_name_to_path(name);
        self.exts.push(ext.clone());

        for (port, dir, typ) in ports {
            let target = format!("{ext}.{port}");
            match dir {
                PortDirection::Incoming => self.paths.insert(target.into(), PathType::Incoming(*typ)),
                PortDirection::Outgoing => self.paths.insert(target.into(), PathType::Outgoing(*typ)),
            };
        }
        self.components.insert(ext, Component::Ext);
        self
    }

    fn regs(&self) -> Vec<Path> {
        let mut result = vec![];
        for (path, typ) in &self.paths {
            if let PathType::Reg(_typ, _reset) = typ {
                result.push(path.clone());
            }
        }
        result
    }

    fn expand_regs(mut self) -> Self {
        let regs: Vec<Path> = self.regs();

        // fix sets (on the right)
        let targets: Vec<Path> = self.wires.keys().cloned().collect();
        for target in targets {
            if regs.contains(&target) {
                let set_path = format!("{target}.set");
                let expr = self.wires.remove(&target).unwrap();
                self.wires.insert(set_path.into(), expr);
            }
        }

        // fix vals (on the left)
        let mut wires: Vec<(Path, Expr)> = vec![];
        for (target, expr) in &self.wires {
            let expr = expr.clone().expand_regs_as_val(&regs);
            wires.push((target.clone(), expr));
        }
        self.wires = wires.into_iter().collect();
        self
    }

    pub fn build(self) -> Circuit {
        Circuit(Arc::new(self.expand_regs()))
    }
}

impl Net {
    fn from(terminal: Path) -> Net {
        Net(terminal, vec![])
    }

    pub fn add(&mut self, terminal: Path) {
        if self.0 != terminal {
            self.1.push(terminal);
            self.1.sort();
            self.1.dedup();
        }
    }

    pub fn driver(&self) -> Path {
        self.0.clone()
    }

    pub fn drivees(&self) -> &[Path] {
        &self.1
    }

    pub fn terminals(&self) -> Vec<Path> {
        let mut results = vec![self.0.clone()];
        for terminal in &self.1 {
            results.push(terminal.clone());
        }
        results
    }

    pub fn contains(&self, terminal: Path) -> bool {
        if terminal == self.0 {
            true
        } else {
            self.1.contains(&terminal)
        }
    }
}

#[derive(Debug, Clone)]
pub enum PathType {
    Node(Type),
    Incoming(Type),
    Outgoing(Type),
    Reg(Type, Value),
}

#[derive(Debug, Clone)]
pub struct Net(Path, Vec<Path>);

#[derive(Debug, Clone, Copy)]
pub enum PortDirection {
    Incoming,
    Outgoing,
}
