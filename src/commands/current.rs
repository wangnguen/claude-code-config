//! `ccc models current`: the model a new Claude Code session in this folder
//! starts on, why, and what each alias resolves to. Reads files only, so it
//! answers instantly and works offline.

use crate::active::{self, Origin, Pick, Sources};
use crate::slots::{self, SLOTS};
use crate::ui;

/// Width of the slot column.
const LABEL: usize = 16;

pub fn run() {
    let sources = Sources::load();
    let active = active::resolve(&sources);
    let values = active::slots(&sources);
    let marked = active::active_slot(&active);

    println!();
    ui::print_header(&ui::ICON_DOC, "Current Model");
    ui::print_row("Starts on", &active.display());
    ui::print_row("Because", &active.why());
    if let Some((saved, origin)) = &active.shadowed {
        ui::print_row("Ignored", &format!("'{saved}' saved by /model ({})", origin.short()));
        ui::print_row("", "ANTHROPIC_MODEL outranks it on every launch");
    }
    ui::print_separator();

    for (i, slot) in SLOTS.iter().enumerate() {
        let label = if marked == Some(i) { format!("{} ◀", slot.label) } else { slot.label.to_string() };
        let detail = match &values[i] {
            Some((value, origin)) => format!("{value}  · {}", origin.short()),
            None if slot.optional => "not set (optional)".to_string(),
            None if i == slots::MAIN => "not set; /model's choice decides".to_string(),
            None => "not set; goes to Anthropic's own model".to_string(),
        };
        ui::print_status_with(status(i, values[i].is_some()), &label, &detail, LABEL);
    }
    ui::print_separator();

    // Saying where it read from matters most when it read nothing of this
    // folder's own: the template ccc copies is never read by Claude Code.
    if !sources.has_local() {
        ui::print_row("This folder", "not set up: run 'ccc lite' or 'ccc models pick'");
    }
    if sources.layers.is_empty() {
        ui::print_row("Read from", "no settings file; shell environment only");
    }
    for layer in &sources.layers {
        ui::print_row(layer_label(layer.origin), &layer.path.display().to_string());
    }
    ui::print_footer();
    println!();
    println!("A /model switch inside a running session is not saved in any file: check it with /status there.");
    println!("Change models with 'ccc models pick'.");
}

fn status(slot: usize, set: bool) -> crate::checks::Status {
    use crate::checks::Status;
    match (set, SLOTS[slot].optional) {
        (true, _) => Status::Pass,
        (false, true) => Status::Info,
        (false, false) => Status::Warn,
    }
}

/// Label for one file that fed the answer, highest precedence first.
fn layer_label(origin: Origin) -> &'static str {
    match origin {
        Origin::Managed => "Managed",
        Origin::Local => "Folder (local)",
        Origin::Project => "Folder (shared)",
        Origin::User => "User",
        Origin::Shell | Origin::BuiltIn => "",
    }
}

/// One line for banners: the model, and where it came from when that is not
/// this folder's own settings.
pub fn banner_line() -> String {
    let active = active::resolve(&Sources::load());
    match (&active.pick, active.origin) {
        (Pick::BuiltIn, _) => format!("{} · built-in default", active.display()),
        (_, Origin::Local) => active.display(),
        (_, origin) => format!("{} · from {}", active.display(), origin.short()),
    }
}
