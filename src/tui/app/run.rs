//! Terminal setup, the input thread, and the draw/handle loop
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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
    let suspended = Arc::new(AtomicBool::new(false));
    let mut model = Model::new(opts, tx.clone(), suspended.clone());
    model.init();

    let mut terminal = ratatui::init();
    if let Ok(size) = terminal.size() {
        model.width = size.width;
        model.height = size.height;
    }

    spawn_input_thread(tx, suspended);

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

        if let Some(path) = model.take_editor_request() {
            open_editor(&mut terminal, &mut model, path);
        }
    };

    ratatui::restore();
    res
}

/// Hands the terminal to the editor and takes it back afterwards.
///
/// Blocking the UI thread is the point: the editor owns the terminal until it exits, and nothing
/// should be drawn over it. The input thread is told to stand down first, because two readers on the
/// same tty would each swallow half the keystrokes.
fn open_editor(terminal: &mut ratatui::DefaultTerminal, model: &mut Model, path: String) {
    let Some((editor, suspended)) = model.editor_command(path) else {
        model.status = "Could not work out which editor to use; check `code.editor`.".to_string();
        return;
    };

    suspended.store(true, Ordering::SeqCst);
    ratatui::restore();

    let status = std::process::Command::new(&editor.program)
        .envs(editor.envs)
        .args(editor.args)
        .status();

    // A fresh terminal starts with an empty back buffer, so the next draw repaints every cell.
    // `Terminal::clear` would look tidier and is a trap: it asks the terminal where the cursor is
    // and waits for the reply, which some terminals never send.
    *terminal = ratatui::init();
    suspended.store(false, Ordering::SeqCst);

    model.status = match status {
        Ok(exit) if exit.success() => String::new(),
        Ok(exit) => format!("{} exited with {exit}", editor.program),
        Err(e) => format!("Could not run {}: {e}", editor.program),
    };
}

/// Turns crossterm events into messages until the channel closes.
fn spawn_input_thread(tx: std::sync::mpsc::Sender<Msg>, suspended: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        loop {
            if suspended.load(Ordering::SeqCst) {
                // The editor is reading this terminal; stay out of its way.
                std::thread::sleep(INPUT_POLL);
                continue;
            }

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
