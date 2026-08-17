//! Terminal setup, the input thread, and the draw/handle loop
use std::time::Duration;

use crossterm::event::{Event, KeyEventKind};

use super::{Model, Msg, Options};
use crate::Result;
use crate::tui::view;

/// How long the input thread waits for a key before looking around.
///
/// It polls rather than blocking in `read()` so that opening an editor can tell it to stand down:
/// two readers on the same terminal would fight over keystrokes.
const INPUT_POLL: Duration = Duration::from_millis(150);

/// Messages drained per frame. Bounded so a burst cannot starve the renderer.
const DRAIN_LIMIT: usize = 128;

/// Runs the terminal UI until the user quits.
pub fn run(opts: Options) -> Result<()> {
    // Anything the TUI borrows from a `Display` impl must arrive without escape sequences; ratatui
    // paints its own colors.
    colored::control::set_override(false);

    let (tx, rx) = std::sync::mpsc::channel::<Msg>();
    let mut model = Model::new(opts, tx.clone());
    model.init();

    let mut terminal = ratatui::init();
    if let Ok(size) = terminal.size() {
        model.width = size.width;
        model.height = size.height;
    }

    spawn_input_thread(tx);

    let res = loop {
        model.ensure_cursor_visible();
        if let Err(e) = terminal.draw(|f| view::draw(&model, f)) {
            break Err(e.into());
        }

        // Blocks until something happens; the UI thread does no work of its own.
        let Ok(msg) = rx.recv() else { break Ok(()) };
        model.handle(msg);
        for queued in rx.try_iter().take(DRAIN_LIMIT) {
            model.handle(queued);
        }

        if model.should_quit() {
            break Ok(());
        }
    };

    ratatui::restore();
    res
}

/// Turns crossterm events into messages until the channel closes.
fn spawn_input_thread(tx: std::sync::mpsc::Sender<Msg>) {
    std::thread::spawn(move || {
        loop {
            match crossterm::event::poll(INPUT_POLL) {
                Ok(false) => continue,
                Err(_) => return,
                Ok(true) => {}
            }

            let msg = match crossterm::event::read() {
                // Release events would double every keypress on terminals that report them.
                Ok(Event::Key(k)) if k.kind != KeyEventKind::Release => Msg::Key(k),
                Ok(Event::Resize(w, h)) => Msg::Resize(w, h),
                Ok(_) => continue,
                Err(_) => return,
            };

            if tx.send(msg).is_err() {
                return;
            }
        }
    });
}
