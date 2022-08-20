use eframe::egui;
use std::thread::JoinHandle;
use tokio::sync::{mpsc::UnboundedSender, watch::Receiver};

pub type Message = Box<[u8]>;

struct Network {
    /// Handle to the network thread.
    handle: JoinHandle<()>,
    /// Unbounded sender (of messages) to the network thread.
    submit: UnboundedSender<Message>,
}

pub struct App {
    /// Information related to the foreign network thread.
    /// Wrapped in an `Option` so that we can join safely.
    network: Option<Network>,
    /// Contents of the user input.
    input: String,
    /// Chat log of previously received messages.
    log: Receiver<String>,
}

impl App {
    pub fn new(
        handle: JoinHandle<()>,
        submit: UnboundedSender<Message>,
        log: Receiver<String>,
    ) -> Self {
        Self { input: String::new(), network: Some(Network { handle, submit }), log }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        use egui::FontId;
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Chat Log");
            ui.label("Note that all communication in this chat room is unencrypted and multi-casted.");
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                for line in self.log.borrow().split_terminator('\n') {
                    let (addr, msg) = line.split_once(' ').unwrap();
                    ui.horizontal(|ui| {
                        use egui::RichText;
                        ui.label(RichText::new(addr).font(FontId::monospace(12.0)).color(egui::Color32::GREEN));
                        ui.label(RichText::new(msg).font(FontId::monospace(12.0)));
                    });
                }
            })
        });
        egui::TopBottomPanel::bottom("user-input").show(ctx, |ui| {
            let widget = egui::TextEdit::singleline(&mut self.input)
                .desired_width(f32::INFINITY)
                .hint_text("Press Enter to send chat message...")
                .font(FontId::proportional(16.0))
                .margin(egui::vec2(8.0, 8.0));
            if ui.add(widget).lost_focus() && ui.input().key_pressed(egui::Key::Enter) {
                let bytes = core::mem::take(&mut self.input).into_bytes().into_boxed_slice();
                self.network.as_ref().unwrap().submit.send(bytes).expect("receiver closed");
            }
        });
    }

    fn on_exit(&mut self) {
        let Network { handle, submit, .. } = self.network.take().unwrap();

        // Dropping the sender half synchronizes with the network
        // thread that it is now time to join with the main thread.
        drop(submit);
        handle.join().expect("failed to join network thread");
    }
}
