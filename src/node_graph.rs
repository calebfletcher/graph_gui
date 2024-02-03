use std::collections::BTreeMap;

use eframe::egui;
use egui::{Color32, Ui};
use egui_snarl::{
    ui::{PinInfo, SnarlViewer},
    InPin, InPinId, NodeId, OutPin, OutPinId, Snarl,
};
use petgraph::Graph;

const STRING_COLOR: Color32 = Color32::from_rgb(0x00, 0xb0, 0x00);
const NUMBER_COLOR: Color32 = Color32::from_rgb(0xb0, 0x00, 0x00);
const UNTYPED_COLOR: Color32 = Color32::from_rgb(0xb0, 0xb0, 0xb0);

#[derive(Clone)]
pub enum DemoNode {
    /// Node with single input.
    /// Displays the value of the input.
    Sink,

    /// Value node with a single output.
    /// The value is editable in UI.
    Number(f64),

    /// Value node with a single output.
    String(String),

    Add {
        res: f64,
    },
}

impl DemoNode {
    fn update(node: NodeId, snarl: &mut Snarl<DemoNode>) {
        match snarl[node] {
            DemoNode::Sink => {}
            DemoNode::Number(_) => {}
            DemoNode::String(_) => {}
            DemoNode::Add { .. } => {
                // Eval addition
                let value: f64 = (0..2)
                    .map(|idx| snarl.in_pin(InPinId { node, input: idx }))
                    .flat_map(|pin| pin.remotes)
                    .map(|pin| match snarl[pin.node] {
                        DemoNode::Sink => unreachable!(),
                        DemoNode::Number(val) => val,
                        DemoNode::String(_) => unreachable!(),
                        DemoNode::Add { res } => res,
                    })
                    .sum();

                // Need to get a new reference to res again
                if let DemoNode::Add { ref mut res } = snarl[node] {
                    *res = value;
                }
            }
        }
    }
}

pub struct DemoViewer;

impl DemoViewer {
    pub fn as_petgraph(snarl: &mut Snarl<DemoNode>) -> Graph<NodeId, ()> {
        let mut graph = petgraph::Graph::<NodeId, ()>::new();

        let mut nodeid_to_idx = BTreeMap::new();

        // Add nodes to graph
        for (node_id, _node) in snarl.node_ids() {
            let idx = graph.add_node(node_id);
            nodeid_to_idx.insert(node_id, idx);
        }

        // Add edges
        for (node_id, node) in snarl.node_ids() {
            let downstream_nodeids = (0..Self.outputs(node))
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

    pub fn evaluate(snarl: &mut Snarl<DemoNode>, _start: Option<NodeId>) {
        let graph = Self::as_petgraph(snarl);
        let mut visitor = petgraph::visit::Topo::new(&graph);
        while let Some(node) = visitor.next(&graph) {
            let id = graph[node];
            DemoNode::update(id, snarl);
        }
    }
}

impl SnarlViewer<DemoNode> for DemoViewer {
    #[inline]
    fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<DemoNode>) {
        // Validate connection
        match (&snarl[from.id.node], &snarl[to.id.node]) {
            (DemoNode::Sink, _) => {
                unreachable!("Sink node has no outputs")
            }
            (_, DemoNode::Sink) => {}
            (_, DemoNode::Number(_)) => {
                unreachable!("Number node has no inputs")
            }
            (_, DemoNode::String(_)) => {
                unreachable!("String node has no inputs")
            }
            (DemoNode::Number(_), DemoNode::Add { .. }) => {}
            (DemoNode::Add { .. }, DemoNode::Add { .. }) => {}
            (_, DemoNode::Add { .. }) => {
                unreachable!("cannot add non-numbers")
            }
        }

        let mut temp_graph = snarl.clone();

        // Remove other connections to this input
        for &remote in &to.remotes {
            temp_graph.disconnect(remote, to.id);
        }

        // Add the new connection
        temp_graph.connect(from.id, to.id);

        // Check for cycles
        if petgraph::algo::is_cyclic_directed(&Self::as_petgraph(&mut temp_graph)) {
            return;
        }
        *snarl = temp_graph;

        // Update the destination node
        DemoNode::update(to.id.node, snarl);

        // Propogate the destination node's value
        Self::evaluate(snarl, Some(to.id.node));
    }

    fn title(&mut self, node: &DemoNode) -> String {
        match node {
            DemoNode::Sink => "Sink".to_owned(),
            DemoNode::Number(_) => "Number".to_owned(),
            DemoNode::String(_) => "String".to_owned(),
            DemoNode::Add { .. } => "Add".to_owned(),
        }
    }

    fn inputs(&mut self, node: &DemoNode) -> usize {
        match node {
            DemoNode::Sink => 1,
            DemoNode::Number(_) => 0,
            DemoNode::String(_) => 0,
            DemoNode::Add { .. } => 2,
        }
    }

    fn outputs(&mut self, node: &DemoNode) -> usize {
        match node {
            DemoNode::Sink => 0,
            DemoNode::Number(_) => 1,
            DemoNode::String(_) => 1,
            DemoNode::Add { .. } => 1,
        }
    }

    fn show_input(
        &mut self,
        pin: &InPin,
        ui: &mut Ui,
        _scale: f32,
        snarl: &mut Snarl<DemoNode>,
    ) -> PinInfo {
        match snarl[pin.id.node] {
            DemoNode::Sink => {
                assert_eq!(pin.id.input, 0, "Sink node has only one input");

                match &*pin.remotes {
                    [] => {
                        ui.label("None");
                        PinInfo::circle().with_fill(UNTYPED_COLOR)
                    }
                    [remote] => match snarl[remote.node] {
                        DemoNode::Sink => unreachable!("Sink node has no outputs"),
                        DemoNode::Number(value) => {
                            assert_eq!(remote.output, 0, "Number node has only one output");
                            ui.label(format_float(value));
                            PinInfo::square().with_fill(NUMBER_COLOR)
                        }
                        DemoNode::String(ref value) => {
                            assert_eq!(remote.output, 0, "String node has only one output");
                            ui.label(format!("{:?}", value));
                            PinInfo::triangle().with_fill(STRING_COLOR)
                        }
                        DemoNode::Add { res } => {
                            ui.label(format_float(res));
                            PinInfo::square().with_fill(NUMBER_COLOR)
                        }
                    },
                    _ => unreachable!("Sink input has only one wire"),
                }
            }
            DemoNode::Number(_) => {
                unreachable!("Number node has no inputs")
            }
            DemoNode::String(_) => {
                unreachable!("String node has no inputs")
            }
            DemoNode::Add { .. } => match &*pin.remotes {
                [] => {
                    ui.label("None");
                    PinInfo::square().with_fill(NUMBER_COLOR)
                }
                [remote] => match snarl[remote.node] {
                    DemoNode::Sink => unreachable!("Sink node has no outputs"),
                    DemoNode::Number(value) => {
                        assert_eq!(remote.output, 0, "Number node has only one output");
                        ui.label(format_float(value));
                        PinInfo::square().with_fill(NUMBER_COLOR)
                    }
                    DemoNode::String(ref value) => {
                        assert_eq!(remote.output, 0, "String node has only one output");
                        ui.label(format!("{:?}", value));
                        PinInfo::triangle().with_fill(STRING_COLOR)
                    }
                    DemoNode::Add { res } => {
                        ui.label(format_float(res));
                        PinInfo::square().with_fill(NUMBER_COLOR)
                    }
                },
                _ => unreachable!("Sink input has only one wire"),
            },
        }
    }

    fn show_output(
        &mut self,
        pin: &OutPin,
        ui: &mut Ui,
        _scale: f32,
        snarl: &mut Snarl<DemoNode>,
    ) -> PinInfo {
        match snarl[pin.id.node] {
            DemoNode::Sink => {
                unreachable!("Sink node has no outputs")
            }
            DemoNode::Number(ref mut value) => {
                assert_eq!(pin.id.output, 0, "Number node has only one output");
                if ui.add(egui::DragValue::new(value)).changed() {
                    Self::evaluate(snarl, Some(pin.id.node));
                }
                PinInfo::square().with_fill(NUMBER_COLOR)
            }
            DemoNode::String(ref mut value) => {
                assert_eq!(pin.id.output, 0, "String node has only one output");
                let edit = egui::TextEdit::singleline(value)
                    .clip_text(false)
                    .desired_width(0.0)
                    .margin(ui.spacing().item_spacing);
                ui.add(edit);
                PinInfo::triangle().with_fill(STRING_COLOR)
            }
            DemoNode::Add { res } => {
                ui.label(format_float(res));
                PinInfo::square().with_fill(NUMBER_COLOR)
            }
        }
    }

    fn input_color(
        &mut self,
        pin: &InPin,
        _style: &egui::Style,
        snarl: &mut Snarl<DemoNode>,
    ) -> Color32 {
        match snarl[pin.id.node] {
            DemoNode::Sink => {
                assert_eq!(pin.id.input, 0, "Sink node has only one input");
                match &*pin.remotes {
                    [] => UNTYPED_COLOR,
                    [remote] => match snarl[remote.node] {
                        DemoNode::Sink => unreachable!("Sink node has no outputs"),
                        DemoNode::Number(_) => NUMBER_COLOR,
                        DemoNode::String(_) => STRING_COLOR,
                        DemoNode::Add { .. } => NUMBER_COLOR,
                    },
                    _ => unreachable!("Sink input has only one wire"),
                }
            }
            DemoNode::Number(_) => {
                unreachable!("Number node has no inputs")
            }
            DemoNode::String(_) => {
                unreachable!("String node has no inputs")
            }
            DemoNode::Add { .. } => NUMBER_COLOR,
        }
    }

    fn output_color(
        &mut self,
        pin: &OutPin,
        _style: &egui::Style,
        snarl: &mut Snarl<DemoNode>,
    ) -> Color32 {
        match snarl[pin.id.node] {
            DemoNode::Sink => {
                unreachable!("Sink node has no outputs")
            }
            DemoNode::Number(_) => NUMBER_COLOR,
            DemoNode::String(_) => STRING_COLOR,
            DemoNode::Add { .. } => NUMBER_COLOR,
        }
    }

    fn graph_menu(
        &mut self,
        pos: egui::Pos2,
        ui: &mut Ui,
        _scale: f32,
        snarl: &mut Snarl<DemoNode>,
    ) {
        ui.label("Add node");
        if ui.button("Number").clicked() {
            snarl.insert_node(pos, DemoNode::Number(0.0));
            ui.close_menu();
        }
        if ui.button("String").clicked() {
            snarl.insert_node(pos, DemoNode::String("".to_owned()));
            ui.close_menu();
        }
        if ui.button("Sink").clicked() {
            snarl.insert_node(pos, DemoNode::Sink);
            ui.close_menu();
        }
        if ui.button("Add").clicked() {
            snarl.insert_node(pos, DemoNode::Add { res: 0. });
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
        snarl: &mut Snarl<DemoNode>,
    ) {
        ui.label("Node menu");
        if ui.button("Remove").clicked() {
            snarl.remove_node(node);
            ui.close_menu();
        }
    }

    fn has_on_hover_popup(&mut self, _: &DemoNode) -> bool {
        true
    }

    fn show_on_hover_popup(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut Ui,
        _scale: f32,
        snarl: &mut Snarl<DemoNode>,
    ) {
        match snarl[node] {
            DemoNode::Sink => {
                ui.label("Displays anything connected to it");
            }
            DemoNode::Number(_) => {
                ui.label("Outputs integer value");
            }
            DemoNode::String(_) => {
                ui.label("Outputs string value");
            }
            DemoNode::Add { .. } => {
                ui.label("Outputs added value");
            }
        }
    }
}

fn format_float(v: f64) -> String {
    let v = (v * 1000.0).round() / 1000.0;
    format!("{}", v)
}
