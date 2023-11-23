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
    components: BTreeMap<Path, Component>,
    wires: BTreeMap<Path, Expr>,
    path: Vec<String>,
}

impl Circuit {
    pub fn new(name: &str) -> CircuitNode {
        let components = vec![("top".into(), Component::Mod)].into_iter().collect();
        CircuitNode {
            components,
            wires: BTreeMap::new(),
            path: vec![name.to_string()],
        }
    }

    pub fn components(&self) -> &BTreeMap<Path, Component> {
        &self.0.components
    }

    pub fn component(&self, path: Path) -> Option<&Component> {
        if let Some(component) = &self.0.components.get(&path) {
            Some(&component)
        } else {
            None
        }
    }

    pub fn wires(&self) -> &BTreeMap<Path, Expr> {
        &self.0.wires
    }

    pub fn exts(&self) -> Vec<Path> {
        let mut result = vec![];
        for (path, typ) in &self.0.components {
            if let Component::Ext = typ {
                result.push(path.clone());
            }
        }
        result
    }

    pub fn regs(&self) -> Vec<Path> {
        let mut result = vec![];
        for (path, typ) in &self.0.components {
            if let Component::Reg(_typ, _reset) = typ {
                result.push(path.clone());
            }
        }
        result
    }

    pub fn reset_for_reg(&self, path: Path) -> Option<Value> {
        let component = self.component(path);
        if let Some(Component::Reg(_typ, reset)) = component {
            Some(reset.clone())
        } else {
            None
        }
    }

    pub fn terminals(&self) -> Vec<Path> {
        let mut terminals = vec![];
        for (path, component) in &self.0.components {
            match component {
                Component::Incoming(_typ) => terminals.push(path.clone()),
                Component::Outgoing(_typ) => terminals.push(path.clone()),
                Component::Node(_typ) => {
                    terminals.push(path.clone());
                },
                Component::Reg(_typ, _reset) => {
                    terminals.push(path.clone());
                    terminals.push(path.set());
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

    fn to_abs_path(&self, name: &str) -> Path {
        let path = self.path.join(".");
        format!("{path}.{name}").into()
    }

    pub(crate) fn node(mut self, name: &str, typ: Type) -> Self {
        let path = self.to_abs_path(name);
        self.components.insert(path.clone(), Component::Node(typ));
        self
    }

    pub(crate) fn incoming(mut self, name: &str, typ: Type) -> Self {
        let path = self.to_abs_path(name);
        self.components.insert(path, Component::Incoming(typ));
        self
    }

    pub(crate) fn outgoing(mut self, name: &str, typ: Type) -> Self {
        let path = self.to_abs_path(name);
        self.components.insert(path, Component::Outgoing(typ));
        self
    }

    pub(crate) fn reg(mut self, name: &str, typ: Type, reset: Value) -> Self {
        let path = self.to_abs_path(name);
        self.components.insert(path, Component::Reg(typ, reset));
        self
    }

    pub(crate) fn wire(mut self, name: &str, expr: &Expr) -> Self {
        let path: Path = self.to_abs_path(name).into();
        self.wires.insert(path, expr.clone().to_absolute(&self.current_path()));
        self
    }

    pub fn instantiate(mut self, name: &str, circuit: &CircuitNode) -> Self {
        let mod_path = self.current_path();
        self = self.push(name);
        self.components.insert(self.current_path(), Component::Mod);

        for (path, component) in &circuit.components {
            if path != &"top".into() {
                let target = mod_path.join(path.clone());
                self.components.insert(target, component.clone());
            }
        }

        for (path, expr) in &circuit.wires {
            let target = mod_path.join(path.clone());
            let expr = expr.clone().to_absolute(&mod_path);
            self.wires.insert(target, expr);
        }
        self = self.pop();
        self
    }

    fn current_path(&self) -> Path {
        self.path.join(".").into()
    }

    pub(crate) fn ext(mut self, name: &str, ports: &[(String, PortDirection, Type)]) -> Self {
        let ext = self.to_abs_path(name);

        for (port, dir, typ) in ports {
            let target = format!("{ext}.{port}");
            match dir {
                PortDirection::Incoming => self.components.insert(target.into(), Component::Incoming(*typ)),
                PortDirection::Outgoing => self.components.insert(target.into(), Component::Outgoing(*typ)),
            };
        }
        self.components.insert(ext, Component::Ext);
        self
    }

    pub fn build(self) -> Circuit {
        for (_path, expr) in &self.wires {
            assert!(expr.clone().is_absolute(), "{expr:?} is not absolute!");
        }
        Circuit(Arc::new(self))
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
