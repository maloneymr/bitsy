use super::*;

use std::time::Duration;
use std::time::SystemTime;

pub type NetId = usize;

pub struct Sim {
    nets: Vec<Net>,
    net_values: Vec<Value>,
    net_id_by_path: BTreeMap<Path, NetId>,
    wires: Box<BTreeMap<Path, (Expr, WireType)>>,
    regs: Box<BTreeSet<Path>>,
    reg_resets: BTreeMap<Path, Value>,
    exts: BTreeMap<Path, Box<dyn ExtInstance>>,
    clock_ticks: u64,
    start_time: SystemTime,
    clock_freq_cap: Option<f64>,
}

impl Sim {
    pub fn new(circuit: &Circuit) -> Sim {
        let nets = nets(circuit);
        let net_values = nets.iter().map(|_net| Value::X).collect();
        let regs: Box<BTreeSet<Path>> = Box::new(
            circuit
                .regs()
                .iter()
                .cloned()
                .collect());
        let reg_resets: BTreeMap<Path, Value> =
            regs
                .iter()
                .cloned()
                .map(|path| (path.clone(), circuit.reset_for_reg(path).unwrap()))
                .collect();
        let wires: Box<BTreeMap<Path, (Expr, WireType)>> = Box::new(
            circuit
                .wires()
                .iter()
                .cloned()
                .map(|Wire(target, expr, wiretype)| (target, (expr, wiretype)))
                .collect());

        let netid_by_path: BTreeMap<Path, NetId> =
            circuit
                .terminals()
                .iter()
                .map(|path| {
                    for (net_id, net) in nets.iter().enumerate() {
                        if net.contains(path.clone()) {
                            return (path.clone(), net_id);
                        }
                    }
                    unreachable!()
                })
                .collect();

        let mut sim = Sim {
            nets,
            net_values,
            net_id_by_path: netid_by_path,
            wires,
            regs,
            reg_resets,
            exts: BTreeMap::new(),
            start_time: SystemTime::now(),
            clock_ticks: 0,
            clock_freq_cap: None,
        };
        sim.broadcast_update_constants();
        sim
    }

    pub fn cap_clock_freq(mut self, freq: f64) -> Self {
        self.clock_freq_cap = Some(freq);
        self
    }

    pub fn ext<P: Into<Path>>(mut self, path: P, ext_inst: Box<dyn ExtInstance>) -> Self {
        self.exts.insert(path.into(), ext_inst);
        self.broadcast_update_constants();
        self
    }

    pub(crate) fn poke_net(&mut self, net_id: NetId, value: Value) {
        self.net_values[net_id] = value;
    }

    pub(crate) fn peek_net(&self, net_id: NetId) -> Value {
        self.net_values[net_id]
    }

    pub fn peek<P: Into<Path>>(&self, path: P) -> Value {
        let path: Path = path.into();

        let terminal_path = path.clone();
        let net_id = self.net_id_by_path[&terminal_path];
        let value = self.net_values[net_id];

        value
    }

    pub fn poke<P: Into<Path>>(&mut self, path: P, value: Value) {
        let path: Path = path.into();

        let net_id = self.net_id_by_path[&path];
        self.net_values[net_id] = value;

        if !self.is_reg(&path) {
            self.broadcast_update(path);
        }
    }

    pub fn set<P: Into<Path>>(&mut self, path: P, value: Value) {
        let path: Path = path.into();

        let net_id = self.net_id_by_path[&path];
        self.net_values[net_id] = value;
        self.broadcast_update(path);
    }

    fn broadcast_update_constants(&mut self) {
        let wires = self.wires.clone();
        for (target, (expr, wire_type)) in wires.iter() {
            let target_terminal: Path = match wire_type {
                WireType::Connect => target.clone(),
                WireType::Latch => target.set(),
            };
            if expr.is_constant() {
                let value = expr.eval(&self);
                self.poke(target_terminal.clone(), value);
            }
        }
    }

    fn broadcast_update(&mut self, terminal: Path) {
        let wires = self.wires.clone();
        for (target, (expr, wire_type)) in wires.iter() {
            let target_terminal: Path = match wire_type {
                WireType::Connect => target.clone(),
                WireType::Latch => target.set(),
            };
            if expr.depends_on(terminal.clone()) {
                let value = expr.eval(&self);
                self.poke(target_terminal.clone(), value);
            }
        }

        let ext_path = terminal.parent();
        if self.exts.contains_key(&ext_path) {
            let value = self.peek(terminal.clone());
            let ext = self.exts.get_mut(&ext_path).unwrap();
            let port_name: PortName = terminal[ext_path.len() + 1..].into();
            if ext.incoming_ports().contains(&port_name) {
                let updates = ext.update(port_name.to_string(), value);
                let poke_values: Vec<(Path, Value)> = updates
                    .into_iter()
                    .map(|(port_name, value)| {
                        let affected_path: Path = format!("{ext_path}.{port_name}").into();
                        (affected_path, value)
                    })
                    .collect();

                for (path, value) in poke_values {
                    self.poke(path, value);
                }
            }
        }
    }

    fn is_reg(&self, path: &Path) -> bool {
        self.regs.contains(&path.parent()) || self.regs.contains(&path)
    }

    pub fn clock(&mut self) {
        self.clock_ticks += 1;

        // frequency cap
        if let Some(clock_freq_cap) = self.clock_freq_cap {
            let mut clock_freq = self.clocks_per_second();
            while clock_freq.is_finite() && clock_freq > clock_freq_cap {
                clock_freq = self.clocks_per_second();
            }
        }

        let regs = self.regs.clone();
        for path in regs.iter() {
            let set_value = self.peek(path.set());
            self.poke(path.clone(), set_value);
        }

        let mut poke_values: Vec<(Path, Value)> = vec![];
        for (ext_path, ext) in &mut self.exts {
            let updates = ext.clock();
            poke_values.extend(updates
                .into_iter()
                .map(|(port_name, value)| {
                    let affected_path: Path = format!("{ext_path}.{port_name}").into();
                    (affected_path, value)
                }));
        }

        for (path, value) in poke_values {
            self.poke(path, value);
        }

        let reg_paths: Vec<Path> = self.regs.iter().cloned().collect();
        for path in reg_paths {
            self.broadcast_update(path);
        }
    }

    pub fn reset(&mut self) {
        let reg_paths: Vec<Path> = self.regs.iter().cloned().collect();
        for path in reg_paths {
            let reset = self.reg_resets[&path];
            self.poke(path, reset);
        }

        for (_path, ext) in &mut self.exts {
            ext.reset();
        }

        let reg_paths: Vec<Path> = self.regs.iter().cloned().collect();
        for path in reg_paths {
            self.broadcast_update(path);
        }
    }

    pub fn clocks_per_second(&self) -> f64 {
        let end_time = SystemTime::now();
        let duration: Duration = end_time.duration_since(self.start_time).unwrap();
        1_000_000.0 * self.clock_ticks as f64 / duration.as_micros() as f64
    }
}

impl std::fmt::Debug for Sim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        for (net_id, value) in self.net_values.iter().enumerate() {
            let net = &self.nets[net_id];
            write!(f, "    {:>5}   ", format!("{value:?}"))?;
            writeln!(f, "{}", net.terminals().iter().map(|t| t.to_string()).collect::<Vec<String>>().join(" "))?;
        }

        Ok(())
    }
}

pub fn nets(circuit: &Circuit) -> Vec<Net> {
    let mut immediate_driver_for: BTreeMap<Path, Path> = BTreeMap::new();

    for Wire(target, expr, wire_type) in circuit.wires() {
        let target_terminal: Path = match wire_type {
            WireType::Connect => target.clone(),
            WireType::Latch => target.set(),
        };
        if let Expr::Reference(driver) = expr {
            immediate_driver_for.insert(target_terminal.clone(), driver.clone());
         }
     }

    let mut drivers: BTreeSet<Path> = BTreeSet::new();
    for terminal in circuit.terminals() {
        drivers.insert(driver_for(terminal, &immediate_driver_for));
    }

    let mut nets: BTreeMap<Path, Net> = BTreeMap::new();
    for driver in &drivers {
        nets.insert(driver.clone(), Net::from(driver.clone()));
    }

    for terminal in circuit.terminals() {
        let driver = driver_for(terminal.clone(), &immediate_driver_for);
        let net = nets.get_mut(&driver).unwrap();
        net.add(terminal);
    }

    let nets: Vec<Net> = nets.values().into_iter().cloned().collect();
    nets
}

fn driver_for(terminal: Path, immediate_driver_for: &BTreeMap<Path, Path>) -> Path {
    let mut driver: &Path = &terminal;
    while let Some(immediate_driver) = &immediate_driver_for.get(driver) {
        driver = immediate_driver;
    }
    driver.clone()
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
pub struct Net(Path, Vec<Path>);
