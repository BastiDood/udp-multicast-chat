use eframe::egui;
use std::thread::JoinHandle;
use tokio::sync::{mpsc::UnboundedSender, watch::Receiver};

pub type Message = Box<[u8]>;

struct Network {
    /// Handle to the network thread.
    handle: JoinHandle<std::io::Result<()>>,
    /// Unbounded sender (of messages) to the network thread.
    submit: UnboundedSender<Message>,
    /// Chat log of previously received messages.
    log: Receiver<String>,
}

pub struct App {
    /// Information related to the foreign network thread.
    /// Wrapped in an `Option` so that we can join safely.
    network: Option<Network>,
    /// Contents of the user input.
    input: String,
}

impl App {
    pub fn new(
        handle: JoinHandle<std::io::Result<()>>,
        submit: UnboundedSender<Message>,
        log: Receiver<String>,
    ) -> Self {
        Self {
            network: Some(Network {
                handle,
                submit,
                log,
            }),
            input: String::new(),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let log = self.network.as_ref().unwrap().log.borrow();
                for line in log.split_terminator('\n') {
                    ui.label(line);
                }
            });
        });
        egui::TopBottomPanel::bottom("user-input").show(ctx, |ui| {
            let widget = egui::TextEdit::singleline(&mut self.input).desired_width(f32::INFINITY);
            if ui.add(widget).lost_focus() && ui.input().key_pressed(egui::Key::Enter) {
                self.input.push('\n');
                let bytes = core::mem::take(&mut self.input)
                    .into_bytes()
                    .into_boxed_slice();
                self.network
                    .as_ref()
                    .unwrap()
                    .submit
                    .send(bytes)
                    .expect("receiver closed");
            }
        });
    }

    fn on_exit(&mut self, _: &eframe::glow::Context) {
        let Network { handle, submit, .. } = self.network.take().unwrap();

        // Dropping the sender half synchronizes with the network
        // thread that it is now time to join with the main thread.
        drop(submit);

        // It is important that the channel is dropped first before joining.
        handle
            .join()
            .expect("failed to join network thread")
            .expect("network thread encountered an I/O error");
    }
}
