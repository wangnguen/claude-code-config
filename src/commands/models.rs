use anyhow::{bail, Result};

use crate::api::{get_api_config, get_current_key, is_vn_model, list_models, VN_SUFFIX};
use crate::ui;
use crate::utils::mask_key;

pub fn run() -> Result<()> {
    let api_key = match get_current_key() {
        Some(key) => key,
        None => bail!("API key not set. Run 'ccc key add' first."),
    };

    let (base_url, _) = get_api_config();

    println!();
    ui::print_header(&ui::ICON_DOC, "Gateway Models");
    ui::print_row("API", &format!("{base_url}/v1/models"));
    ui::print_row("Key", &mask_key(&api_key));
    ui::print_separator();

    let sp = ui::spinner("Fetching model list...");
    let result = list_models(&api_key);
    sp.finish_and_clear();

    let models = match result {
        Ok(models) if models.is_empty() => {
            ui::print_check(false, "Result", "gateway returned no models");
            ui::print_footer();
            println!();
            return Ok(());
        }
        Ok(models) => models,
        Err(e) => {
            ui::print_check(false, "Result", &e);
            ui::print_footer();
            println!();
            return Ok(());
        }
    };

    // The gateway can list models the key is not entitled to, so mark which
    // ones are safe to select rather than printing a flat list.
    for id in &models {
        let usable = is_vn_model(id);
        let detail = if usable { "usable" } else { "not for this key" };
        ui::print_check(usable, id, detail);
    }

    let vn = models.iter().filter(|m| is_vn_model(m)).count();
    ui::print_separator();
    ui::print_row("Total", &models.len().to_string());
    ui::print_row(&format!("With {VN_SUFFIX}"), &vn.to_string());
    ui::print_footer();
    println!();

    if vn == 0 {
        println!("No {VN_SUFFIX} model found. Check that you are using your own virtual key.");
    } else {
        println!("Pick a {VN_SUFFIX} model in Claude Code via /model.");
    }

    Ok(())
}
