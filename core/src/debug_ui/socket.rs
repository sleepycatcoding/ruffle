use crate::context::UpdateContext;
use egui::{Grid, Window};

#[derive(Debug, Default)]
pub struct SocketListWindow {}

impl SocketListWindow {
    pub fn show(&mut self, egui_ctx: &egui::Context, context: &mut UpdateContext) -> bool {
        let mut keep_open = true;

        Window::new("Socket List")
            .open(&mut keep_open)
            .show(egui_ctx, |ui| {
                Grid::new("socket_list_grid").num_columns(3).show(ui, |ui| {
                    ui.strong("A");
                    ui.strong("B");
                    ui.strong("C");
                    ui.end_row();

                    for socket in context.sockets.open_sockets() {
                        ui.label("a");
                        ui.label("b");
                        ui.label("c");
                        ui.end_row();
                    }
                });
            });

        keep_open
    }
}
