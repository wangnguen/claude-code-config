use crate::api::get_api_config;
use crate::config::VERSION;
use crate::ui;

pub fn run() {
    // Deliberately no gateway URL here: the banner is the one thing people
    // screenshot and paste into chat.
    let (_, model) = get_api_config();

    println!();
    ui::print_brand(&format!("Claude Code Config {VERSION}"), &[model.as_str()]);
    println!();
}
