use std::collections::{BTreeMap, BTreeSet};

use eframe::egui;
use egui::{Color32, Ui};
use egui_snarl::{
    ui::{PinInfo, SnarlViewer},
    InPin, InPinId, NodeId, OutPin, OutPinId, Snarl,
};
use petgraph::{visit::Walker, Graph};

const STRING_COLOR: Color32 = Color32::from_rgb(0x00, 0xb0, 0x00);
const NUMBER_COLOR: Color32 = Color32::from_rgb(0xb0, 0x00, 0x00);
const UNTYPED_COLOR: Color32 = Color32::from_rgb(0xb0, 0xb0, 0xb0);

pub enum TypedData {
    Number(f64),
    String(String),
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataType {
    Number,
    String,
    Unknown,
}

impl DataType {
    fn colour(&self) -> Color32 {
        match self {
            DataType::Number => NUMBER_COLOR,
            DataType::String => STRING_COLOR,
            DataType::Unknown => UNTYPED_COLOR,
        }
    }

    fn pin_info(&self) -> PinInfo {
        let info = match self {
            DataType::Number => PinInfo::square(),
            DataType::String => PinInfo::triangle(),
            DataType::Unknown => PinInfo::circle(),
        };
        info.with_fill(self.colour())
    }

    fn compatible_with(&self, destination: DataType) -> bool {
        if *self == destination {
            return true;
        }
        if destination == DataType::Unknown {
            return true;
        }
        false
    }
}

pub trait Node {
    fn name(&self) -> String;
    fn inputs(&self) -> Vec<DataType>;
    fn outputs(&self) -> Vec<DataType>;
    /// Returns none if the value is not available yet
    fn output_value(&self, idx: usize) -> Option<TypedData> {
        let _ = idx;
        unimplemented!()
    }
    fn update(&mut self, inputs: &[TypedData]) {
        let _ = inputs;
    }
    /// Return true if the node should be recalculated
    fn show_input(&mut self, idx: usize, remote: Option<TypedData>, ui: &mut Ui) -> bool {
        let _ = (idx, remote, ui);
        false
    }
    /// Return true if the node should be recalculated
    fn show_output(&mut self, idx: usize, ui: &mut Ui) -> bool {
        let _ = (idx, ui);
        false
    }
}

#[derive(Debug, Clone)]
pub struct NumberNode {
    value: f64,
}

impl NumberNode {
    pub fn new(value: f64) -> Self {
        Self { value }
    }
}

impl Node for NumberNode {
    fn name(&self) -> String {
        "Number".to_owned()
    }

    fn inputs(&self) -> Vec<DataType> {
        Vec::new()
    }

    fn outputs(&self) -> Vec<DataType> {
        vec![DataType::Number]
    }

    fn output_value(&self, idx: usize) -> Option<TypedData> {
        assert_eq!(idx, 0);
        Some(TypedData::Number(self.value))
    }

    fn show_output(&mut self, idx: usize, ui: &mut Ui) -> bool {
        assert_eq!(idx, 0);
        if ui.add(egui::DragValue::new(&mut self.value)).changed() {
            // TODO: Evaluate and propagate
            return true;
        }
        false
    }
}

#[derive(Debug, Clone, Default)]
pub struct AddNode {
    cached_result: Option<f64>,
}

impl Node for AddNode {
    fn name(&self) -> String {
        "Add".to_owned()
    }

    fn inputs(&self) -> Vec<DataType> {
        vec![DataType::Number, DataType::Number]
    }

    fn outputs(&self) -> Vec<DataType> {
        vec![DataType::Number]
    }

    fn output_value(&self, idx: usize) -> Option<TypedData> {
        assert_eq!(idx, 0);
        self.cached_result.map(TypedData::Number)
    }

    fn show_input(&mut self, idx: usize, remote: Option<TypedData>, ui: &mut Ui) -> bool {
        assert!(idx < 2);
        let Some(remote) = remote else {
            return false;
        };
        match remote {
            TypedData::Number(val) => ui.label(format_float(val)),
            _ => unimplemented!(),
        };
        false
    }

    fn show_output(&mut self, idx: usize, ui: &mut Ui) -> bool {
        assert_eq!(idx, 0);
        if let Some(res) = self.cached_result {
            ui.label(format_float(res));
        }
        false
    }

    fn update(&mut self, inputs: &[TypedData]) {
        self.cached_result = Some(
            inputs
                .iter()
                .filter_map(|v| {
                    if let TypedData::Number(v) = v {
                        Some(*v)
                    } else {
                        None
                    }
                })
                .sum(),
        );
    }
}

#[derive(Debug, Clone)]
pub struct SinkNode;

impl Node for SinkNode {
    fn name(&self) -> String {
        "Sink".to_owned()
    }

    fn inputs(&self) -> Vec<DataType> {
        vec![DataType::Number]
    }

    fn outputs(&self) -> Vec<DataType> {
        Vec::new()
    }

    fn show_input(&mut self, idx: usize, remote: Option<TypedData>, ui: &mut Ui) -> bool {
        assert_eq!(idx, 0);
        let Some(remote) = remote else {
            return false;
        };
        match remote {
            TypedData::Number(val) => ui.label(format_float(val)),
            _ => unimplemented!(),
        };
        false
    }
}

pub struct DemoViewer;

impl DemoViewer {
    pub fn as_petgraph(snarl: &mut Snarl<Box<dyn Node>>) -> Graph<NodeId, ()> {
        let mut graph = petgraph::Graph::<NodeId, ()>::new();

        let mut nodeid_to_idx = BTreeMap::new();

        // Add nodes to graph
        for (node_id, _node) in snarl.node_ids() {
            let idx = graph.add_node(node_id);
            nodeid_to_idx.insert(node_id, idx);
        }

        // Add edges
        for (node_id, node) in snarl.node_ids() {
            let downstream_nodeids = (0..DemoViewer.outputs(node))
                .map(|i| {
                    snarl.out_pin(OutPinId {
                        node: node_id,
                        output: i,
                    })
                })
                .flat_map(|output| output.remotes)
                .map(|inpin| inpin.node);

            for downstream in downstream_nodeids {
                graph.add_edge(nodeid_to_idx[&node_id], nodeid_to_idx[&downstream], ());
            }
        }

        graph
    }

    pub fn evaluate(snarl: &mut Snarl<Box<dyn Node>>, start: Option<NodeId>) {
        let graph = Self::as_petgraph(snarl);

        // TODO: Replace this with a more efficient filtered toposort with
        // a specified starting point
        let node_filter = match start {
            Some(initial) => {
                let initial = graph
                    .node_indices()
                    .find(|idx| graph[*idx] == initial)
                    .unwrap();

                // Find all the nodes downstream of this one
                let bfs = petgraph::visit::Bfs::new(&graph, initial);
                let downstream_nodes = bfs.iter(&graph).collect::<BTreeSet<_>>();
                Some(downstream_nodes)
            }
            None => None,
        };
        let mut visitor = petgraph::visit::Topo::new(&graph);

        // Visit every node in topological order
        while let Some(node) = visitor.next(&graph) {
            // If there is a filter, only include nodes that are in the filter
            if let Some(filter) = &node_filter {
                if !filter.contains(&node) {
                    continue;
                }
            }
            // Update the node
            let id = graph[node];
            let inputs = snarl[id]
                .inputs()
                .into_iter()
                .enumerate()
                .map(|(i, _)| snarl.in_pin(InPinId { node: id, input: i }))
                .map(|inpin| {
                    assert_eq!(inpin.remotes.len(), 1);
                    let remote = inpin.remotes[0];
                    snarl[remote.node].output_value(remote.output)
                })
                .collect::<Option<Vec<_>>>();
            if let Some(inputs) = inputs {
                // All inputs are connected
                snarl[id].update(&inputs);
            }
        }
    }
}

impl SnarlViewer<Box<dyn Node>> for DemoViewer {
    fn show_header(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut Ui,
        _scale: f32,
        _snarl: &mut Snarl<Box<dyn Node>>,
    ) {
        //ui.label(self.title(&snarl[node]));
        ui.label(format!("ID: {}", node.0));
    }

    fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<Box<dyn Node>>) {
        let from_node = &snarl[from.id.node];
        let to_node = &snarl[to.id.node];

        // Validate connection
        assert!(from.id.output < from_node.outputs().len());
        assert!(to.id.input < to_node.inputs().len());
        assert!(from_node.outputs()[from.id.output].compatible_with(to_node.inputs()[to.id.input]));

        // Remove other connections to this input
        for &remote in &to.remotes {
            snarl.disconnect(remote, to.id);
        }

        // Add the new connection
        snarl.connect(from.id, to.id);

        // Check for cycles
        if petgraph::algo::is_cyclic_directed(&Self::as_petgraph(snarl)) {
            return;
        }

        // Update the destination node
        let inputs = snarl[to.id.node]
            .inputs()
            .into_iter()
            .enumerate()
            .map(|(i, _)| {
                snarl.in_pin(InPinId {
                    node: to.id.node,
                    input: i,
                })
            })
            .map(|inpin| {
                assert_eq!(inpin.remotes.len(), 1);
                let remote = inpin.remotes[0];
                snarl[remote.node].output_value(remote.output)
            })
            .collect::<Option<Vec<_>>>();
        if let Some(inputs) = inputs {
            // All inputs are connected
            snarl[to.id.node].update(&inputs);
        }

        // Propogate the destination node's value
        Self::evaluate(snarl, Some(to.id.node));
    }

    fn title(&mut self, node: &Box<dyn Node>) -> String {
        node.name()
    }

    fn inputs(&mut self, node: &Box<dyn Node>) -> usize {
        node.inputs().len()
    }

    fn outputs(&mut self, node: &Box<dyn Node>) -> usize {
        node.outputs().len()
    }

    fn show_input(
        &mut self,
        pin: &InPin,
        ui: &mut Ui,
        _scale: f32,
        snarl: &mut Snarl<Box<dyn Node>>,
    ) -> PinInfo {
        assert!(pin.remotes.len() <= 1);
        let remote = pin
            .remotes
            .first()
            .and_then(|remote| snarl[remote.node].output_value(remote.output));
        let should_update = snarl[pin.id.node].show_input(pin.id.input, remote, ui);
        if should_update {
            Self::evaluate(snarl, Some(pin.id.node));
        }
        snarl[pin.id.node].inputs()[pin.id.input].pin_info()
    }

    fn show_output(
        &mut self,
        pin: &OutPin,
        ui: &mut Ui,
        _scale: f32,
        snarl: &mut Snarl<Box<dyn Node>>,
    ) -> PinInfo {
        let should_update = snarl[pin.id.node].show_output(pin.id.output, ui);
        if should_update {
            Self::evaluate(snarl, Some(pin.id.node));
        }
        snarl[pin.id.node].outputs()[pin.id.output].pin_info()
    }

    fn input_color(
        &mut self,
        pin: &InPin,
        _style: &egui::Style,
        snarl: &mut Snarl<Box<dyn Node>>,
    ) -> Color32 {
        snarl[pin.id.node].inputs()[pin.id.input].colour()
    }

    fn output_color(
        &mut self,
        pin: &OutPin,
        _style: &egui::Style,
        snarl: &mut Snarl<Box<dyn Node>>,
    ) -> Color32 {
        snarl[pin.id.node].outputs()[pin.id.output].colour()
    }

    fn graph_menu(
        &mut self,
        pos: egui::Pos2,
        ui: &mut Ui,
        _scale: f32,
        snarl: &mut Snarl<Box<dyn Node>>,
    ) {
        ui.label("Add node");
        if ui.button("Number").clicked() {
            snarl.insert_node(pos, Box::new(NumberNode::new(0.)));
            ui.close_menu();
        }
        if ui.button("Sink").clicked() {
            snarl.insert_node(pos, Box::new(SinkNode));
            ui.close_menu();
        }
        if ui.button("Add").clicked() {
            snarl.insert_node(pos, Box::<AddNode>::default());
            ui.close_menu();
        }
    }

    fn node_menu(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut Ui,
        _scale: f32,
        snarl: &mut Snarl<Box<dyn Node>>,
    ) {
        ui.label("Node menu");
        if ui.button("Remove").clicked() {
            snarl.remove_node(node);
            ui.close_menu();
        }
    }
}

fn format_float(v: f64) -> String {
    let v = (v * 1000.0).round() / 1000.0;
    format!("{}", v)
}
