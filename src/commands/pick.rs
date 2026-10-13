//! `ccc models pick`: choose this folder's models one question at a time,
//! with the arrow keys and Enter, in the terminal's own scrollback. Every
//! choice is written straight away, so there is no "save" step to forget and
//! Esc never loses anything. The full-screen picker is still in `ccc ui`.

use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::slice;

use anyhow::{bail, Result};
use console::{measure_text_width, style, truncate_str, Term};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input, Select};
use serde_json::Value;

use crate::api::{is_listed, list_models_at, usable_models, ApiError, KeySource};
use crate::config::{
    env_value, load_local_for_edit, set_auth_token, write_json, KeysStore, ENV_AUTH_TOKEN,
    ENV_BASE_URL, ENV_GATEWAY_DISCOVERY,
};
use crate::project::ProjectState;
use crate::slots::{self, CUSTOM, SLOTS};
use crate::ui;
use crate::utils::{ensure_local_settings_ignored, mask_key};

type Values = [Option<String>; 5];

/// Column the value starts at in the slot menu.
const LABEL: usize = 11;
/// The menu entry after the slots.
const DISCOVERY_ITEM: usize = SLOTS.len();

pub fn run() -> Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        bail!("'models pick' needs an interactive terminal. Use 'ccc models set <slot> <model>' instead.");
    }
    let store = KeysStore::load();
    let project = ProjectState::load(&store);
    let Some((key_value, source)) = project.key.clone() else {
        bail!("No API key yet. Add one with 'ccc key add <name>' first.");
    };
    let key_name = project.key_name.clone().unwrap_or_else(|| mask_key(&key_value));

    println!();
    ui::print_header(&ui::ICON_DOC, "Pick Models");
    ui::print_row("Folder", &project.folder);
    ui::print_row("Key", &project.key_label());
    ui::print_row("Gateway", &project.base_url);

    let sp = ui::spinner("Asking the gateway which models this key can use...");
    let listed = list_models_at(&project.base_url, &key_value);
    sp.finish_and_clear();
    let models = match listed {
        Ok(models) => models,
        Err(e) => {
            ui::print_check(false, "Models", &e.to_string());
            ui::print_footer();
            println!();
            match e {
                ApiError::Rejected(_) => {
                    bail!("The gateway refused '{key_name}'. Switch keys with 'ccc key use', then try again.")
                }
                _ => bail!("Couldn't load the model list. 'ccc check' shows what is wrong."),
            }
        }
    };
    let usable = usable_models(&models, &project.filter);
    ui::print_row("Usable", &usable_summary(models.len(), usable.len(), &project.filter));
    ui::print_footer();
    println!();
    if usable.is_empty() {
        bail!("This key can use none of the models the gateway lists. Check that it is your own key.");
    }

    let mut picker = Picker {
        theme: ColorfulTheme::default(),
        values: project.slots.clone(),
        discovery: project.discovery,
        usable,
        base_url: project.base_url.clone(),
        key_name,
        key_value,
        folder_has_key: source == KeySource::Folder,
        save_key: None,
        saved_to: None,
    };
    picker.offer_fill()?;
    picker.menu()?;
    picker.finish();
    Ok(())
}

fn usable_summary(listed: usize, usable: usize, filter: &str) -> String {
    if filter.is_empty() || usable == listed {
        format!("{usable} models")
    } else {
        format!("{usable} of {listed} listed (ids ending in {filter})")
    }
}

struct Picker {
    theme: ColorfulTheme,
    /// What this folder's settings hold right now, re-read after every write.
    values: Values,
    discovery: bool,
    usable: Vec<String>,
    base_url: String,
    key_name: String,
    key_value: String,
    /// Whether this folder's settings already carry the key the list came from.
    folder_has_key: bool,
    /// Asked on the first write when the folder has no key of its own.
    save_key: Option<bool>,
    saved_to: Option<PathBuf>,
}

impl Picker {
    /// A fresh folder, or one set up for another key, has slots that cannot
    /// work. Fixing them all is one question rather than four.
    fn offer_fill(&mut self) -> Result<()> {
        let broken = broken_slots(&self.values, &self.usable);
        if broken.is_empty() {
            return Ok(());
        }
        let names: Vec<&str> = broken.iter().map(|&i| SLOTS[i].label).collect();
        let fill = Confirm::with_theme(&self.theme)
            .with_prompt(format!(
                "{}: not set, or not available to this key. Fill with the newest matching models?",
                names.join(", ")
            ))
            .default(true)
            .interact_opt()?
            .unwrap_or(false);
        if !fill {
            return Ok(());
        }
        let usable = self.usable.clone();
        let mut changes = Vec::new();
        self.save(|json| changes = slots::fit(json, &usable))?;
        for change in changes {
            report(change.slot, Some(&change.to));
        }
        Ok(())
    }

    fn menu(&mut self) -> Result<()> {
        let mut cursor = broken_slots(&self.values, &self.usable).first().copied().unwrap_or(0);
        loop {
            let items = menu_items(&self.values, &self.usable, self.discovery, item_width());
            let choice = Select::with_theme(&self.theme)
                .with_prompt("Choose a slot to change (Esc when done)")
                .items(&items)
                .default(cursor)
                .report(false)
                .interact_opt()?;
            match choice {
                Some(i) if i < SLOTS.len() => {
                    cursor = i;
                    self.pick(i)?;
                }
                Some(DISCOVERY_ITEM) => {
                    cursor = DISCOVERY_ITEM;
                    self.toggle_discovery()?;
                }
                _ => return Ok(()),
            }
        }
    }

    fn pick(&mut self, slot: usize) -> Result<()> {
        let spec = &SLOTS[slot];
        let current = self.values[slot].clone();
        let choices = slots::ordered_for(slot, &self.usable);
        let mut items = model_items(&choices, current.as_deref(), item_width());
        if slot == CUSTOM {
            items.push("Type another id…".to_string());
            items.push("None — no extra /model entry".to_string());
        }
        let default = current
            .as_deref()
            .and_then(|v| choices.iter().position(|m| is_listed(v, slice::from_ref(m))))
            .unwrap_or(0);
        let Some(choice) = Select::with_theme(&self.theme)
            .with_prompt(format!("{} — {}", spec.label, spec.help))
            .items(&items)
            .default(default)
            .report(false)
            .interact_opt()?
        else {
            return Ok(());
        };

        let value = if let Some(model) = choices.get(choice) {
            if slots::supports_1m(model) {
                let wants_1m = current.as_deref().is_some_and(slots::has_1m);
                let Some(one_m) = self.ask_1m(model, wants_1m)? else {
                    return Ok(());
                };
                Some(slots::with_1m(model, one_m))
            } else {
                Some(model.clone())
            }
        } else if choice == choices.len() {
            let typed: String = Input::with_theme(&self.theme)
                .with_prompt("Model id for the Custom entry")
                .with_initial_text(current.clone().unwrap_or_default())
                .allow_empty(true)
                .interact_text()?;
            let typed = typed.trim().to_string();
            if slots::has_1m(&typed) && !slots::supports_1m(&typed) {
                println!("  {} Only Sonnet and Opus models have a 1M variant; drop the [1m].", style("✗").red());
                return Ok(());
            }
            (!typed.is_empty()).then_some(typed)
        } else {
            None
        };

        if value == current {
            println!("  {} {} unchanged", style("·").dim(), spec.label);
            return Ok(());
        }
        self.save(|json| slots::write(json, slot, value.as_deref()))?;
        report(slot, value.as_deref());
        Ok(())
    }

    /// None when the question was dismissed.
    fn ask_1m(&self, model: &str, wants_1m: bool) -> Result<Option<bool>> {
        let items = ["Standard context", "1M context (adds [1m])"];
        Ok(Select::with_theme(&self.theme)
            .with_prompt(format!("Context window for {model}"))
            .items(&items)
            .default(usize::from(wants_1m))
            .report(false)
            .interact_opt()?
            .map(|i| i == 1))
    }

    fn toggle_discovery(&mut self) -> Result<()> {
        let on = !self.discovery;
        self.save(|json| set_discovery(json, on))?;
        let what = if on {
            "on: /model lists every model this key can use"
        } else {
            "off: /model shows only the slots"
        };
        println!("  {} Discovery {what}", style("✓").green());
        Ok(())
    }

    /// Apply one change to this folder's settings and write them.
    fn save(&mut self, edit: impl FnOnce(&mut Value)) -> Result<()> {
        if !self.folder_has_key && self.save_key.is_none() {
            let answer = Confirm::with_theme(&self.theme)
                .with_prompt(format!(
                    "This folder has no key of its own. Save '{}' here too, so Claude Code can reach the gateway?",
                    self.key_name
                ))
                .default(true)
                .interact_opt()?
                .unwrap_or(false);
            self.save_key = Some(answer);
        }

        let (path, mut json) = load_local_for_edit()?;
        edit(&mut json);
        // Models belong to a gateway; a folder that names models but no
        // gateway would send them wherever the next settings file points.
        if env_value(&json, ENV_BASE_URL).is_none() {
            json["env"][ENV_BASE_URL] = Value::String(self.base_url.clone());
        }
        if self.save_key == Some(true) && !self.folder_has_key {
            set_auth_token(&mut json, &self.key_value);
            self.folder_has_key = true;
        }
        write_json(&path, &json)?;
        if env_value(&json, ENV_AUTH_TOKEN).is_some()
            && let Some(note) = ensure_local_settings_ignored()
        {
            println!("  {note}");
        }

        self.values = slots::read_all(&json);
        self.discovery = json["env"][ENV_GATEWAY_DISCOVERY].as_str() == Some("1");
        self.saved_to = Some(path);
        Ok(())
    }

    fn finish(&self) {
        println!();
        match &self.saved_to {
            Some(path) => println!("Saved to {}. Restart Claude Code to use the new models.", path.display()),
            None => println!("Nothing changed."),
        }
        if self.save_key == Some(false) {
            println!("This folder still has no key: run 'ccc key use' before starting Claude Code.");
        }
    }
}

fn report(slot: usize, value: Option<&str>) {
    let label = SLOTS[slot].label;
    match value {
        Some(value) => println!("  {} {label:<7} → {value}", style("✓").green()),
        None => println!("  {} {label} cleared", style("✓").green()),
    }
}

/// Room for one menu line: the theme puts a two-column pointer in front, and
/// a line that wraps would break the menu's redraw.
fn item_width() -> usize {
    (Term::stdout().size().1 as usize).saturating_sub(4).max(24)
}

fn fit(text: &str, width: usize) -> String {
    if measure_text_width(text) <= width {
        text.to_string()
    } else {
        truncate_str(text, width, "…").into_owned()
    }
}

/// Required slots that are empty or point at a model this key cannot use.
fn broken_slots(values: &Values, usable: &[String]) -> Vec<usize> {
    (0..SLOTS.len())
        .filter(|&i| !SLOTS[i].optional && !values[i].as_deref().is_some_and(|v| is_listed(v, usable)))
        .collect()
}

/// One line per slot with what it holds, then discovery, then Done.
fn menu_items(values: &Values, usable: &[String], discovery: bool, width: usize) -> Vec<String> {
    let mut items: Vec<String> = SLOTS
        .iter()
        .enumerate()
        .map(|(i, slot)| {
            let value = match &values[i] {
                Some(v) if slot.optional || is_listed(v, usable) => v.clone(),
                Some(v) => format!("{v}  ✗ not available to this key"),
                None if slot.optional => "none (an optional extra /model entry)".to_string(),
                None => "✗ not set".to_string(),
            };
            fit(&format!("{:<LABEL$}{value}", slot.label), width)
        })
        .collect();
    let state = if discovery {
        "on · /model lists every model this key can use"
    } else {
        "off · /model shows only the slots above"
    };
    items.push(fit(&format!("{:<LABEL$}{state}", "Discovery"), width));
    items.push("Done".to_string());
    items
}

/// The models offered for a slot, with the one it holds now marked.
fn model_items(choices: &[String], current: Option<&str>, width: usize) -> Vec<String> {
    choices
        .iter()
        .map(|model| {
            let line = match current {
                Some(v) if is_listed(v, slice::from_ref(model)) && slots::has_1m(v) => {
                    format!("{model}  ← current, 1M")
                }
                Some(v) if is_listed(v, slice::from_ref(model)) => format!("{model}  ← current"),
                _ => model.clone(),
            };
            fit(&line, width)
        })
        .collect()
}

fn set_discovery(json: &mut Value, on: bool) {
    if on {
        json["env"][ENV_GATEWAY_DISCOVERY] = Value::String("1".into());
    } else if let Some(env) = json["env"].as_object_mut() {
        env.remove(ENV_GATEWAY_DISCOVERY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn usable() -> Vec<String> {
        ["claude-haiku-4-5-vn", "claude-opus-5-vn", "claude-sonnet-5-vn"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    #[test]
    fn a_fresh_folder_has_every_required_slot_broken() {
        let values: Values = Default::default();
        assert_eq!(broken_slots(&values, &usable()), vec![0, 1, 2, 3]);
    }

    #[test]
    fn a_slot_the_key_cannot_use_is_broken_and_the_1m_marker_is_fine() {
        let values: Values = [
            Some("claude-sonnet-5-vn[1m]".into()),
            Some("claude-sonnet-5-vn".into()),
            Some("claude-opus-5".into()),
            Some("claude-haiku-4-5-vn".into()),
            Some("anything-at-all".into()),
        ];
        assert_eq!(broken_slots(&values, &usable()), vec![2]);
    }

    #[test]
    fn menu_says_what_each_slot_holds_and_what_is_wrong() {
        let values: Values = [Some("claude-sonnet-5-vn[1m]".into()), None, Some("claude-opus-5".into()), None, None];
        let items = menu_items(&values, &usable(), false, 100);
        assert_eq!(items.len(), SLOTS.len() + 2);
        assert!(items[0].starts_with("Main") && items[0].ends_with("claude-sonnet-5-vn[1m]"), "{}", items[0]);
        assert!(items[1].contains("✗ not set"), "{}", items[1]);
        assert!(items[2].contains("✗ not available"), "{}", items[2]);
        assert!(items[CUSTOM].contains("optional"), "{}", items[CUSTOM]);
        assert!(items[DISCOVERY_ITEM].starts_with("Discovery") && items[DISCOVERY_ITEM].contains("off"));
        assert_eq!(items.last().map(String::as_str), Some("Done"));
    }

    #[test]
    fn menu_lines_never_outgrow_the_terminal() {
        let long = format!("claude-{}-vn", "x".repeat(80));
        let values: Values = [Some(long.clone()), Some(long.clone()), Some(long.clone()), Some(long), None];
        for item in menu_items(&values, &usable(), true, 40) {
            assert!(measure_text_width(&item) <= 40, "{item}");
        }
    }

    #[test]
    fn model_list_marks_the_current_pick() {
        let choices = usable();
        let items = model_items(&choices, Some("claude-sonnet-5-vn[1m]"), 100);
        assert_eq!(items[2], "claude-sonnet-5-vn  ← current, 1M");
        assert_eq!(items[0], "claude-haiku-4-5-vn");
        let items = model_items(&choices, Some("claude-opus-5-vn"), 100);
        assert_eq!(items[1], "claude-opus-5-vn  ← current");
    }

    #[test]
    fn discovery_is_written_as_1_and_removed_when_off() {
        let mut json = json!({});
        set_discovery(&mut json, true);
        assert_eq!(json["env"][ENV_GATEWAY_DISCOVERY], "1");
        set_discovery(&mut json, false);
        assert!(json["env"].get(ENV_GATEWAY_DISCOVERY).is_none());
    }

    #[test]
    fn summary_mentions_the_filter_only_when_it_hides_something() {
        assert_eq!(usable_summary(6, 6, "-vn"), "6 models");
        assert_eq!(usable_summary(9, 6, "-vn"), "6 of 9 listed (ids ending in -vn)");
        assert_eq!(usable_summary(9, 9, ""), "9 models");
    }
}
