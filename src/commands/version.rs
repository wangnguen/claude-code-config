use crate::config::VERSION;
use crate::release;
use crate::ui;

pub fn run() {
    // Deliberately no gateway URL here: the banner is the one thing people
    // screenshot and paste into chat. The model is the one a session started
    // here would use, with its source named when that is not this folder.
    let model = super::current::banner_line();
    let update = release::available_update().map(|latest| format!("Update {latest} is out — run ccc update"));

    println!();
    ui::print_brand(&format!("Claude Code Config {VERSION}"), &[model.as_str()], update.as_deref());
    println!();
}
