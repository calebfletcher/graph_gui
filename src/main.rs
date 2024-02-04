use std::collections::{HashMap, HashSet};

use eframe::{
    egui::{self, Label, Layout, Margin},
    epaint::{Color32, Rounding},
};
use egui_snarl::{
    ui::{BackgroundPattern, SnarlStyle},
    Snarl,
};
use egui_tiles::{Container, Linear, LinearDir, Tile};

mod execution_engine;
mod node_graph;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Task Execution Engine",
        options,
        Box::new(|_cc| Box::<MyApp>::default()),
    )
}

pub enum Pane {
    Config,
    Nodes,
    Statistics,
}

impl Pane {
    pub fn ui(&mut self, ui: &mut egui::Ui) -> egui_tiles::UiResponse {
        ui.label("some pane");
        let dragged = ui
            .allocate_rect(ui.max_rect(), egui::Sense::drag())
            .on_hover_cursor(egui::CursorIcon::Grab)
            .dragged();
        if dragged {
            egui_tiles::UiResponse::DragStarted
        } else {
            egui_tiles::UiResponse::None
        }
    }
}

struct TreeBehavior<'a> {
    snarl: &'a mut Snarl<node_graph::DemoNode>,
    style: &'a SnarlStyle,
    task_execution: &'a mut Option<execution_engine::TaskDag>,
}

impl<'a> egui_tiles::Behavior<Pane> for TreeBehavior<'a> {
    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        pane: &mut Pane,
    ) -> egui_tiles::UiResponse {
        // Title bar
        let (response, painter) = ui.allocate_painter(
            egui::Vec2::new(ui.available_width(), 20.),
            egui::Sense::click_and_drag(),
        );
        painter.rect_filled(painter.clip_rect(), Rounding::same(3.), Color32::LIGHT_GRAY);

        // Title
        let label_resp = ui
            .child_ui(
                painter.clip_rect(),
                Layout::left_to_right(egui::Align::Center),
            )
            .add(Label::new(self.tab_title_for_pane(pane)));

        match pane {
            Pane::Config => {}
            Pane::Nodes => {
                self.snarl.show(
                    &mut node_graph::DemoViewer,
                    self.style,
                    egui::Id::new("snarl"),
                    ui,
                );
            }
            Pane::Statistics => {
                egui::Frame::central_panel(ui.style()).show(ui, |ui| {
                    if ui.button("Calculate Task Dag").clicked() {
                        let graph = node_graph::DemoViewer::as_petgraph(self.snarl);
                        *self.task_execution = Some(execution_engine::TaskDag::new(&graph))
                    }

                    if let Some(task_dag) = self.task_execution {
                        let ready_tasks = task_dag.ready_tasks().collect::<HashSet<_>>();
                        let blocked_tasks = task_dag.blocked_tasks().collect::<HashSet<_>>();
                        for (id, _node) in self.snarl.node_ids() {
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(format!("Task ID: {}", id.0));
                                    ui.separator();
                                    if ready_tasks.contains(&id) {
                                        if ui.button("Complete").clicked() {
                                            let _res = task_dag.complete_task(id);
                                            // TODO: Do something with the newly ready tasks
                                        }
                                    } else if blocked_tasks.contains(&id) {
                                        ui.label("Blocked");
                                    } else {
                                        ui.label("Completed");
                                    }
                                })
                            });
                        }
                    }
                });
            }
        }

        // Allow dragging from the title bar
        if response
            .union(label_resp)
            .on_hover_cursor(egui::CursorIcon::Grab)
            .dragged_by(egui::PointerButton::Primary)
        {
            egui_tiles::UiResponse::DragStarted
        } else {
            egui_tiles::UiResponse::None
        }
    }

    fn tab_title_for_pane(&mut self, pane: &Pane) -> egui::WidgetText {
        match pane {
            Pane::Config => "Config".into(),
            Pane::Nodes { .. } => "Nodes".into(),
            Pane::Statistics => "Statistics".into(),
        }
    }
}

struct MyApp {
    tree: egui_tiles::Tree<Pane>,
    snarl: Snarl<node_graph::DemoNode>,
    style: SnarlStyle,
    task_execution: Option<execution_engine::TaskDag>,
}

impl Default for MyApp {
    fn default() -> Self {
        let mut tiles = egui_tiles::Tiles::default();

        let snarl = Snarl::new();
        let mut style = SnarlStyle::new();
        style.downscale_wire_frame = true;
        style.bg_pattern = BackgroundPattern::Grid(egui_snarl::ui::Grid {
            angle: 0.,
            ..Default::default()
        });

        let config_pane = tiles.insert_pane(Pane::Config);
        let nodes_pane = tiles.insert_pane(Pane::Nodes);
        let stats_pane = tiles.insert_pane(Pane::Statistics);

        let mut inner = Linear {
            children: vec![config_pane, nodes_pane, stats_pane],
            dir: LinearDir::Horizontal,
            ..Default::default()
        };
        inner.shares.set_share(config_pane, 1.);
        inner.shares.set_share(nodes_pane, 3.);
        inner.shares.set_share(stats_pane, 1.);
        let root = tiles.insert_new(Tile::Container(Container::Linear(inner)));

        let tree = egui_tiles::Tree::new("tree", root, tiles);

        Self {
            tree,
            snarl,
            style,
            task_execution: None,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close)
                    }
                    if ui.button("Export Graph").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Graph File", &["dot"])
                            .save_file()
                        {
                            let graph = node_graph::DemoViewer::as_petgraph(&mut self.snarl);

                            // Write to file
                            std::fs::write(
                                path,
                                format!("{:?}", petgraph::dot::Dot::with_config(&graph, &[])),
                            )
                            .unwrap();
                        }
                    }
                    if ui.button("Eval").clicked() {
                        node_graph::DemoViewer::evaluate(&mut self.snarl, None);
                    }
                });

                egui::widgets::global_dark_light_mode_switch(ui);
            });
        });

        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(&ctx.style()).inner_margin(Margin::ZERO))
            .show(ctx, |ui| {
                self.tree.ui(
                    &mut TreeBehavior {
                        snarl: &mut self.snarl,
                        style: &self.style,
                        task_execution: &mut self.task_execution,
                    },
                    ui,
                );
            });
    }
}

#[allow(dead_code)]
fn series_parallel(graph: &petgraph::prelude::Graph<egui_snarl::NodeId, ()>) {
    // Create map of all nodes and their dependencies
    let mut data = HashMap::new();
    for idx in graph.node_indices() {
        let node_deps = graph
            .neighbors_directed(idx, petgraph::Direction::Incoming)
            .map(|idx| graph[idx])
            .collect::<HashSet<_>>();
        data.insert(graph[idx], node_deps);
    }

    loop {
        // Find all dependents with no outstanding dependencies
        let ordered = data
            .iter()
            .filter_map(|(k, v)| v.is_empty().then_some(*k))
            .collect::<HashSet<_>>();
        // If there is none remaining, break
        if ordered.is_empty() {
            break;
        }

        let mut temp_ordered = ordered.iter().copied().collect::<Vec<_>>();
        temp_ordered.sort_unstable();
        println!("{:?}", temp_ordered);

        data = data
            .into_iter()
            .filter(|(k, _v)| !ordered.contains(k))
            .map(|(k, v)| (k, v.difference(&ordered).copied().collect()))
            .collect();
    }
    if !data.is_empty() {
        panic!("cyclic graph");
    }
}
