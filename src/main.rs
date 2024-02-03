use eframe::{
    egui::{self, Label, Layout, Margin},
    epaint::{Color32, Rounding},
};
use egui_snarl::{
    ui::{BackgroundPattern, SnarlStyle},
    Snarl,
};
use egui_tiles::{Container, Linear, LinearDir, Tile};

mod node_graph;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([800.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "egui_tiles example",
        options,
        Box::new(|_cc| Box::<MyApp>::default()),
    )
}

pub enum Pane {
    Config,
    Nodes {
        snarl: Snarl<node_graph::DemoNode>,
        style: SnarlStyle,
    },
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

struct TreeBehavior;

impl egui_tiles::Behavior<Pane> for TreeBehavior {
    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        pane: &mut Pane,
    ) -> egui_tiles::UiResponse {
        let (response, painter) = ui.allocate_painter(
            egui::Vec2::new(ui.available_width(), 20.),
            egui::Sense::click_and_drag(),
        );
        painter.rect_filled(painter.clip_rect(), Rounding::same(3.), Color32::LIGHT_GRAY);

        let label_resp = ui
            .child_ui(
                painter.clip_rect(),
                Layout::left_to_right(egui::Align::Center),
            )
            .add(Label::new(self.tab_title_for_pane(pane)));

        match pane {
            Pane::Config => {}
            Pane::Nodes { snarl, style } => {
                snarl.show(
                    &mut node_graph::DemoViewer,
                    style,
                    egui::Id::new("snarl"),
                    ui,
                );
            }
            Pane::Statistics => {}
        }

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
        let nodes_pane = tiles.insert_pane(Pane::Nodes { snarl, style });
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

        Self { tree }
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
                });
                ui.add_space(16.0);

                egui::widgets::global_dark_light_mode_switch(ui);
            });
        });

        egui::CentralPanel::default()
            .frame(egui::Frame {
                inner_margin: Margin::ZERO,
                ..egui::Frame::central_panel(&ctx.style())
            })
            .show(ctx, |ui| {
                self.tree.ui(&mut TreeBehavior, ui);
            });
    }
}
