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

/// Library functions for handling ZFS

use color_eyre::{Result, Section};
use eyre::{eyre, Context};
use std::{env, io::BufRead, process::Command};

#[cfg_attr(test, derive(PartialEq, Debug))]
pub enum SnapRange<'a> {
    Single(&'a str),
    Range(&'a str, &'a str),
}

#[derive(Default)]
pub struct ReclaimResult {
    pub destroys: Vec<String>,
    pub bytes: usize,
}

pub fn snap_range_commandline(ranges: &[SnapRange]) -> String {
    ranges
        .iter()
        .map(|range| match range {
            SnapRange::Single(snap) => String::from(*snap),
            SnapRange::Range(from, to) => format!("{from}%{to}"),
        })
        .collect::<Vec<String>>()
        .join(",")
}

fn zfs_command() -> Command {
    Command::new(env::var("ZSNAPFREE_ZFS").unwrap_or("zfs".to_string()))
}

pub fn get_snapshots(dataset: &str) -> Result<Vec<String>> {
    let args = ["list", "-Ht", "snapshot", dataset];
    let output = zfs_command()
        .args(args)
        .output()
        .wrap_err_with(|| format!("Failed to run zfs {:?}", args))?;

    if !output.status.success() {
        return Err(eyre!(
            "Failed to fetch snapshots for {dataset}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    snapshots_from_output(dataset, &output.stdout)
}

fn snapshots_from_output(dataset: &str, stdout: &Vec<u8>) -> Result<Vec<String>> {
    let prefix = format!("{dataset}@");

    stdout
        .lines()
        .map(|line_result| match line_result {
            Ok(line) => line
                .split_once('\t')
                .ok_or_else(|| eyre!("Unexpected zfs output line: {line:?}"))
                .map(|s| s.0.to_string()),
            Err(e) => Err(eyre!(e)),
        })
        .map(|full_snapshot_name| {
            let Ok(full_snapshot_name) = full_snapshot_name else {
                return full_snapshot_name;
            };
            let Some(snapshot_name) = full_snapshot_name.strip_prefix(&prefix) else {
                return Err(eyre!(
                    "Invalid snapshot name {full_snapshot_name:?}, expected {prefix}..."
                ));
            };
            Ok(snapshot_name.to_string())
        })
        .collect::<Result<Vec<String>>>()
}

pub fn get_reclaim(dataset: &str, ranges: &[SnapRange]) -> Result<ReclaimResult> {
    let destroy_spec = format!("{dataset}@{}", snap_range_commandline(ranges));
    let args = ["destroy", "-np", &destroy_spec];
    let output = zfs_command()
        .args(args)
        .output()
        .wrap_err_with(|| format!("Failed to run zfs {:?}", args))?;

    if !output.status.success() {
        return Err(eyre!(
            "Failed to dry-run destroy {destroy_spec}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let mut destroys = vec![];

    for line in output.stdout.lines() {
        const RECLAIM_PREFIX: &str = "reclaim\t";
        const DESTROY_PREFIX: &str = "destroy\t";
        let line = &line?;
        if let Some(bytes) = line.strip_prefix(RECLAIM_PREFIX) {
            return Ok(ReclaimResult {
                destroys,
                bytes: bytes.parse()?,
            });
        } else if let Some(destroy) = line.strip_prefix(DESTROY_PREFIX) {
            destroys.push(destroy.to_string());
        };
    }
    Err(
        eyre!("Unexpected output for zfs destroy -np {destroy_spec} - missing 'reclaim' line.")
            .with_note(|| {
                format!(
                    "Output is:\n{}",
                    String::from_utf8_lossy(&output.stdout).trim()
                )
            }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use pretty_assertions::assert_eq;

    #[test]
    fn snap_range_commandline() {
        let data = vec![
            SnapRange::Single("snap1"),
            SnapRange::Range("snap3", "snap7"),
        ];

        assert_eq!(super::snap_range_commandline(&data), "snap1,snap3%snap7");
    }

    #[test]
    fn snapshots_from_output() {
        let stdout = indoc! {"
tank/my_filesystem@zfs-auto-snap_monthly-2023-09-01-0552	22.5M	-	274M	-
tank/my_filesystem@zfs-auto-snap_monthly-2023-10-01-0552	8.85M	-	293M	-
tank/my_filesystem@zfs-auto-snap_monthly-2023-11-01-0652	2.58M	-	309M	-
tank/my_filesystem@zfs-auto-snap_monthly-2023-12-01-0652	2.55M	-	309M	-
"}
        .into();

        assert_eq!(
            super::snapshots_from_output("tank/my_filesystem", &stdout).unwrap(),
            vec![
                "zfs-auto-snap_monthly-2023-09-01-0552",
                "zfs-auto-snap_monthly-2023-10-01-0552",
                "zfs-auto-snap_monthly-2023-11-01-0652",
                "zfs-auto-snap_monthly-2023-12-01-0652",
            ],
        );

        let want_error = super::snapshots_from_output("tank/some_other_filesystem", &stdout);

        if want_error.is_ok() {
            panic!("Wanted 'wrong filesystem', got {:?}", want_error)
        }
    }
}
