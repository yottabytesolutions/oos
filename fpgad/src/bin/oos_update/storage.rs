// SPDX-License-Identifier: GPL-2.0-or-later

use super::args::Args;
use crate::{die, need_data, put, state_path, update, STDOUT_CLOSED};

mod recovered;

pub(crate) fn run(args: &Args) {
    match args.command() {
        "storage-write" => return write(args).unwrap_or_else(|e| die(&e)),
        "storage-data" => return prepare_data(args).unwrap_or_else(|e| die(&e)),
        _ => {}
    }
    need_data(args);
    let execute = args.flag("--execute");
    let mut card = args.open_card(execute).unwrap_or_else(|e| die(&e));
    let state = state_path(args);
    let result = if args.command() == "sd-mode" {
        match args.rest.get(1).map(String::as_str) {
            None => update::sd::restage(&mut card, &state, execute),
            Some("hs") => update::sd::restage_preference(&mut card, &state, Some(update::sd::Mode::Hs), execute),
            Some("sdr104") => {
                update::sd::restage_preference(&mut card, &state, Some(update::sd::Mode::Sdr104), execute)
            }
            Some(_) => Err("sd-mode expects hs or sdr104".into()),
        }
    } else {
        update::rollback(&mut card, &state, "by hand", execute, None)
    };
    match result {
        Ok(report) => report.lines.iter().for_each(|line| out!("{}", line)),
        Err(error) => die(&error),
    }
}

fn prepare_data(args: &Args) -> Result<(), String> {
    prepare_data_at(args, std::path::Path::new("/oos"))
}

fn prepare_data_at(args: &Args, data: &std::path::Path) -> Result<(), String> {
    oos_hal::system::check_data_write(data).map_err(|error| error.to_string())?;
    if args.rest.get(1).is_some_and(|arg| arg == "clean-recovered") {
        let mounts = std::fs::read_to_string("/proc/mounts").map_err(|e| e.to_string())?;
        if !oos_hal::system::oos_mounted(&mounts) {
            return Err("the data area is unmounted".into());
        }
        let (removed, kept) = recovered::clean(data).map_err(|e| e.to_string())?;
        if removed != 0 || kept != 0 {
            out!("{removed} redundant files removed from lost+found, {kept} recovered entries kept");
        }
    } else if let Some(notice) = args.open_card(true)?.prepare_data()? {
        out!("{notice}");
    }
    Ok(())
}

/// Boot helpers submit bytes to the same HAL boundary as signed updates.
fn write(args: &Args) -> Result<(), String> {
    use oos_hal::update::card::{check_range, IMAGE_MAX};
    use std::io::Read;
    let area = args.rest.get(1).ok_or("storage-write AREA OFFSET --card TARGET --execute")?;
    let offset = args.rest.get(2).ok_or("missing absolute byte offset")?.parse::<u64>().map_err(|e| e.to_string())?;
    check_range(area, offset, 1)?;
    if !args.flag("--execute") {
        return Err("storage-write requires --execute".into());
    }
    if args.val("--host-card").is_some() && args.val("--card").is_some() {
        return Err("select either --card or --host-card".into());
    }
    let mut data = Vec::new();
    std::io::stdin().take(IMAGE_MAX as u64 + 1).read_to_end(&mut data).map_err(|e| e.to_string())?;
    if data.len() > IMAGE_MAX {
        return Err("storage payload exceeds the image limit: nothing written".into());
    }
    let mut card = match args.val("--host-card") {
        Some(path) => oos_hal::update::card::open_removable(std::path::Path::new(path))?,
        None => args.open_card(true)?,
    };
    card.write_checked(area, offset, &data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oos_hal::system::admission_test::with_closed_data;

    #[test]
    fn closed_data_refuses_recovery_cleanup_and_card_repair() {
        with_closed_data(module_path!(), "closed_data_refuses_recovery_cleanup_and_card_repair", |data, _| {
            let card = data.join("absent-card");
            for command in ["clean-recovered", "repair"] {
                let args = Args::parse_from(
                    ["storage-data", command, "--card", card.to_str().unwrap()].into_iter().map(str::to_owned),
                );
                assert_eq!(prepare_data_at(&args, data).unwrap_err(), "/oos is closed for shutdown");
            }
        });
    }
}
