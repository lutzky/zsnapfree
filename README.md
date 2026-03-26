<!--
Copyright (C) 2024 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: Apache-2.0
-->

# zsnapfree

This is a TUI for showing how much space can be reclaimed by freeing zfs
snapshots. It is a TUI wrapper over the standard `zfs` tool.

It's useful for cases of "stored big files, deleted them, now a specific set of
snapshots need to be removed to reclaim that space". See
[blog post](https://lutzky.net/posts/zsnapfree/) for more details.

[![asciicast](https://asciinema.org/a/673276.svg)](https://asciinema.org/a/673276)

## Installation

### Prebuilt binaries

See [releases](https://github.com/lutzky/zsnapfree/releases).

### From source

```shell
cargo install --git https://github.com/lutzky/zsnapfree
```

## Running

If your ZFS dataset is named `mypool`:

```shell
zsnapfree mypool
```

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for details.

## License

Apache 2.0; see [`LICENSE`](LICENSE) for details.

## Disclaimer

This project is not an official Google project. It is not supported by
Google and Google specifically disclaims all warranties as to its quality,
merchantability, or fitness for a particular purpose.
