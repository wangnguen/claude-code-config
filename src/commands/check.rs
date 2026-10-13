use anyhow::{anyhow, Result};

use crate::checks::{self, Report, Status};
use crate::ui;

pub fn run() -> Result<()> {
    let sp = ui::plain_spinner("Asking the gateway for your model list...");
    let report = checks::run();
    sp.finish_and_clear();
    let report = report.map_err(|e| anyhow!(e))?;

    print_report("API Connection Check", &report);

    let auth_failed = report
        .groups
        .iter()
        .flatten()
        .any(|item| item.label == "Auth" && item.status == Status::Fail);
    if auth_failed {
        println!("The gateway refused the key. See your keys with 'ccc key list'; switch with 'ccc key use <name>'.");
    } else if report.count(Status::Fail) > 0 {
        println!("Fix the models with 'ccc models pick', or start over with 'ccc lite'.");
    }
    Ok(())
}

fn print_report(title: &str, report: &Report) {
    println!();
    ui::print_header(&ui::ICON_SEARCH, title);
    for (label, value) in &report.rows {
        ui::print_row(label, value);
    }
    for group in &report.groups {
        ui::print_separator();
        for item in group {
            ui::print_status(item.status, &item.label, &item.detail);
        }
    }
    ui::print_separator();
    ui::print_result_line(
        report.count(Status::Pass),
        report.count(Status::Warn),
        report.count(Status::Fail),
    );
    ui::print_footer();
    println!();
}
