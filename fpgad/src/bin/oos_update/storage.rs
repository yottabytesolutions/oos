// SPDX-License-Identifier: GPL-2.0-or-later

use super::args::Args;
use crate::{die, need_data, put, state_path, update, STDOUT_CLOSED};

pub(crate) fn run(args: &Args) {
    match args.command() {
        "storage-write" => return write(args).unwrap_or_else(|e| die(&e)),
        "storage-data" => {
            return args.open_card(true).and_then(|mut card| card.prepare_data()).unwrap_or_else(|e| die(&e))
        }
        _ => {}
    }
    need_data(args);
    let execute = args.flag("--execute");
    let mut card = args.open_card(execute).unwrap_or_else(|e| die(&e));
    let state = state_path(args);
    let result = if args.command() == "sd-mode" {
        update::sd::restage(&mut card, &state, execute)
    } else {
        update::rollback(&mut card, &state, "by hand", execute, None)
    };
    match result {
        Ok(report) => report.lines.iter().for_each(|line| out!("{}", line)),
        Err(error) => die(&error),
    }
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
