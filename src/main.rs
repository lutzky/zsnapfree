// Copyright (C) 2024 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0

//! Main entrypoint for zsnapfree

use clap::Parser;
use color_eyre::Result;
use human_bytes::human_bytes;
use indoc::printdoc;

mod app;
mod tui;
mod zfs;

/// TUI for showing how much space can be reclaimed by freeing zfs snapshots
#[derive(Parser)]
#[command(version, about)]
struct Args {
    target: String,
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let args = Args::parse();

    let mut app = app::App::new(&args.target)?;
    let mut terminal = tui::init()?;
    let app_result = app.run(&mut terminal);

    tui::restore()?;

    app.recalculate_result();
    printdoc!(
        "
      Running the following command should pretend to delete {} snapshots and
      show that this would reclaim {}:

      {}

      run it as root and without `-n` to actually do it.
    ",
        app.result.destroys.len(),
        human_bytes(app.result.bytes as f64),
        app.equivalent_command_line(),
    );

    app_result
}
