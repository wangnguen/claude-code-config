use crate::config::{KeysStore, VERSION};
use crate::project::ProjectState;
use crate::ui::{self, MenuItem};

/// The path through the commands rather than all of them; `help` still lists
/// the full set. Each line says what the command does, because "key use" and
/// "key default" mean nothing until someone has been told the difference.
const SECTIONS: &[(&str, &[MenuItem])] = &[
    (
        "Setup",
        &[
            ("lite", "set up this folder for the team gateway"),
            ("init", "copy ccc's config template into this folder"),
            ("permission", "allow the usual tools without prompts"),
        ],
    ),
    (
        "Keys",
        &[
            ("key", "open the key manager"),
            ("key add <name> <key>", "save a key under a name"),
            ("key use [name]", "make this folder use a saved key"),
            ("key default [name]", "key `init` puts in new folders"),
        ],
    ),
    (
        "Models",
        &[
            ("models", "list the models your key can use"),
            ("models pick", "choose this folder's models"),
        ],
    ),
    (
        "Check",
        &[
            ("check", "test key, models and discovery"),
            ("doctor", "inspect the whole setup"),
            ("ui", "everything above, full-screen"),
        ],
    ),
];

/// Header of the interactive shell: wordmark, where this folder stands, and
/// where to start.
pub fn print() {
    let store = KeysStore::load();
    let project = ProjectState::load(&store);
    let key = format!("Key    {}", project.key_label());
    let (ready, total) = project.slots_ready();
    let models = if project.has_local {
        format!("Models {ready}/{total} slots set")
    } else {
        "Models not set here — run lite or models pick".to_string()
    };
    ui::print_splash(
        &format!("Claude Code Config {VERSION}"),
        &[key.as_str(), models.as_str()],
        SECTIONS,
    );
    ui::print_rule();
    ui::print_hint("? this list · help all commands · clear wipes the screen · exit to leave");
}

/// Sign-off for the interactive shell, Ctrl-C included: a prompt that simply
/// stops answering reads like a crash, one line saying we meant to stop does
/// not.
pub fn print_farewell() {
    ui::print_rule();
    ui::print_hint("Goodbye · run ccc any time");
    println!();
}
