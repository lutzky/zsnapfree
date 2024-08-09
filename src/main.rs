// Copyright 2024 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// Main entrypoint for zsnapfree

use color_eyre::Result;
use human_bytes::human_bytes;
use indoc::printdoc;
use std::env::args;

mod app;
mod tui;
mod zfs;

fn main() -> Result<()> {
    color_eyre::install()?;

    let target = args().nth(1).unwrap();

    let mut terminal = tui::init()?;
    let mut app = app::App::new(&target);
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
