use crate::api::get_api_config;
use crate::config::VERSION;
use crate::ui;

/// The path through the commands rather than all of them; `help` still lists
/// the full set.
const SECTIONS: &[(&str, &[&str])] = &[
    ("Setup", &["lite", "init", "permission"]),
    ("Keys", &["key", "key use"]),
    ("Check", &["check", "models", "doctor"]),
];

/// Header of the interactive shell: wordmark, what this is, and where to start.
pub fn print() {
    let (_, model) = get_api_config();
    ui::print_splash(
        &format!("Claude Code Config {VERSION}"),
        &[model.as_str()],
        SECTIONS,
    );
    ui::print_rule();
    ui::print_hint("? for this list · help for every command · clear to tidy up · exit to leave");
}

/// Sign-off for the interactive shell, Ctrl-C included: a prompt that simply
/// stops answering reads like a crash, one line saying we meant to stop does
/// not.
pub fn print_farewell() {
    ui::print_rule();
    ui::print_hint("Goodbye · run ccc any time");
    println!();
}
