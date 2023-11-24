use super::*;

use std::time::Duration;
use std::time::SystemTime;

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct NetId(usize);

pub struct Sim {
    pub circuit: Circuit,
    exts: BTreeMap<Path, Box<dyn ExtInstance>>,
    nets: Vec<Net>,
    net_values: BTreeMap<NetId, Value>,
    clock_ticks: u64,
    start_time: SystemTime,
    clock_freq_cap: Option<f64>,
}

impl Sim {
    pub fn new(circuit: &Circuit) -> Sim {
        let nets = circuit.nets();
        let net_values = nets.iter().enumerate().map(|(net_id, _net)| (NetId(net_id), Value::X)).collect();

        let mut nettle = Sim {
            circuit: circuit.clone(),
            exts: BTreeMap::new(),
            nets,
            net_values,
            start_time: SystemTime::now(),
            clock_ticks: 0,
            clock_freq_cap: None,
        };
        nettle.broadcast_update_constants();
        nettle
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

    fn net_id_for(&self, terminal: Path) -> NetId {
        for (net_id, net) in self.nets.iter().enumerate() {
            if net.contains(terminal.clone()) {
                return NetId(net_id);
            }
        }
        panic!("No net found for terminal: {terminal:?}")
    }

    fn net_for(&mut self, terminal: Path) -> &mut Net {
        let NetId(net_id) = self.net_id_for(terminal);
        &mut self.nets[net_id]
    }

    pub fn peek<P: Into<Path>>(&self, path: P) -> Value {
        let path: Path = path.into();

        let terminal_path = path.clone();
        let net_id = self.net_id_for(terminal_path);
        let value = self.net_values[&net_id];

        value
    }

    pub fn poke<P: Into<Path>>(&mut self, path: P, value: Value) {
        let path: Path = path.into();

        let net_id = self.net_id_for(path.clone());
        self.net_values.insert(net_id, value);

        if !self.is_reg(&path) {
            self.broadcast_update(path);
        }
    }

    pub fn set<P: Into<Path>>(&mut self, path: P, value: Value) {
        let path: Path = path.into();

        let net_id = self.net_id_for(path.clone());
        self.net_values.insert(net_id, value);
        self.broadcast_update(path);
    }

    fn broadcast_update_constants(&mut self) {
        let wires = self.circuit.wires().clone();
        for (target_terminal, expr) in &wires {
            if expr.is_constant() {
                let value = expr.eval(&self);
                self.poke(target_terminal.clone(), value);
            }
        }
    }

    fn broadcast_update(&mut self, terminal: Path) {
        let wires = self.circuit.wires().clone();
        for (target_terminal, expr) in &wires {
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
        if let Some(Component::Reg(_name, _typ, _reset)) = self.circuit.component(path.parent()) {
            true
        } else if let Some(Component::Reg(_name, _typ, _reset)) = self.circuit.component(path.clone()) {
            true
        } else if let Some(_) = self.circuit.component(path.clone()) {
            false
        } else {
            panic!("is_reg({path}) failed")
        }
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

        for path in self.circuit.regs() {
            let set_value = self.peek(path.set());
            self.poke(path, set_value);
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

        for path in self.circuit.regs() {
            self.broadcast_update(path);
        }
    }

    pub fn reset(&mut self) {
        for path in self.circuit.regs() {
            let reset = self.circuit.reset_for_reg(path.clone()).unwrap();
            self.poke(path, reset);
        }

        for (_path, ext) in &mut self.exts {
            ext.reset();
        }

        for path in self.circuit.regs() {
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
        for (NetId(net_id), value) in &self.net_values {
            let net = &self.nets[*net_id];
            write!(f, "    {:>5}   ", format!("{value:?}"))?;
            writeln!(f, "{}", net.terminals().iter().map(|t| t.to_string()).collect::<Vec<String>>().join(" "))?;
        }

        Ok(())
    }
}
