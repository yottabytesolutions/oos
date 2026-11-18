// SPDX-License-Identifier: GPL-2.0-or-later

use super::args::Args;
use crate::{die, need_data, open_card, put, state_path, update, STDOUT_CLOSED};

pub(crate) fn run(args: &Args) {
    need_data(args);
    let execute = args.flag("--execute");
    let mut card = open_card(args, execute);
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
