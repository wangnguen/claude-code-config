use crate::api::get_api_config;
use crate::config::VERSION;
use crate::release;
use crate::ui;

pub fn run() {
    // Deliberately no gateway URL here: the banner is the one thing people
    // screenshot and paste into chat.
    let (_, model) = get_api_config();
    let update = release::available_update().map(|latest| format!("Update {latest} is out — run ccc update"));

    println!();
    ui::print_brand(&format!("Claude Code Config {VERSION}"), &[model.as_str()], update.as_deref());
    println!();
}
