# ss-resource-fetcher

[![CI](https://github.com/MakoStar/ss-resource-fetcher/actions/workflows/ci.yml/badge.svg)](https://github.com/MakoStar/ss-resource-fetcher/actions/workflows/ci.yml)
English | [简体中文](README.md)

A multi-region resource fetcher for StellaSora. Pipeline: fetch manifest → AES-128-CBC decrypt → protobuf decode → extract patch manifest → download → bsdiff merge. Covers CN / EN / JP / KR / TW.

## Features

- Three independent pipelines: `client` resources, `launcher` pack, `uncensor` pack
- Downloads and merges by tier, restoring the full dependent patch chain to the latest version
- Hash verification plus a cross-run cache: re-running never re-downloads or re-merges
- Platform aware: switch `DEFAULT_PLATFORM` between win / and / ios, output is split by platform
- Any config item can be overridden from the command line, no file editing required

## Installation

Requires Rust 1.85+ (edition 2024).

```bash
git clone https://github.com/MakoStar/ss-resource-fetcher.git
cd ss-resource-fetcher
cargo build --release
```

The binary lands at `target/release/ResourceFetcher`, shortened to `ResourceFetcher` below.

## Usage

```bash
ResourceFetcher client                 # full pipeline: fetch → download → merge → version JSON
ResourceFetcher client -r              # dump the manifest only, no download
ResourceFetcher launcher -a            # every launcher resource
ResourceFetcher uncensor -r CN         # the CN uncensor pack
```

Running it bare only prints the help. Three subcommands, three independent pipelines:

| Subcommand | Purpose |
| --- | --- |
| `client` | Regular resource pipeline (download + merge + version JSON) |
| `launcher` | Download the launcher pack |
| `uncensor` | Download the uncensor pack |

`launcher` requires an explicit scope (`-a` / `-p` / `-f` / `-e`, pick one); `uncensor` has three modes: region, default URL and custom list.

## Configuration

Built-in defaults are used and **nothing is written** by default. Two ways to change config:

**Temporary override** — `-s SECTION.KEY=VALUE`, this run only, repeatable:

```bash
ResourceFetcher client -s DEFAULT_REGION=JP
ResourceFetcher client -s DEFAULT_PLATFORM=and            # win / and / ios
ResourceFetcher client -s PATCH_TAGS=[u,n]                # include whole-file patches
ResourceFetcher -d TASK.CLIENT.SERVERS.CN client          # remove a config item
```

**Persistent config** — generate the file first, then edit it:

```bash
ResourceFetcher --gen_config        # writes ss-resource-fetcher.toml (overwrites)
ResourceFetcher --print_keys        # every key with its type and default
```

The config file is TOML, organised into `GLOBAL` / `NETWORK` / `PATCH_UPDATE_TAG` / `TASK.CLIENT` / `TASK.LAUNCHER` / `TASK.UNCENSOR`. Arrays take brackets (`-s 'FILES=[a,b]'`) and repeating a key appends. Platform-related fields store the real value for the current platform, so switching `DEFAULT_PLATFORM` makes the next load follow along.

## Command-line options

`--help` is the source of truth:

```text
$ ResourceFetcher --help
Multi-region resource fetcher for StellaSora: fetch, decrypt, decode and merge hotfix patches.

Usage: ResourceFetcher [OPTIONS] [COMMAND]

Commands:
  client    Client resource pipeline
  launcher  Download the launcher pack
  uncensor  Download the uncensor pack
  help      Print this message or the help of the given subcommand(s)

Options:
  -s, --set <KEY=VALUE>        Override a config item, repeatable
  -g, --gen_config[=<BOOL>]    Write the config file, unset=skip, true=generate (overwrites) [possible values: true, false]
  -a, --add_comments [<LANG>]  Write comments into the generated config, cn or en [possible values: cn, en]
      --log_level <LEVEL>      Log level: off, error, warn, info, debug, trace [default: info]
  -p, --print_keys             Print every -s KEY=VALUE key with type and default
  -d, --del <KEY>              Remove a config item, repeatable
  -h, --help                   Print help
  -V, --version                Print version
```

See `ResourceFetcher <subcommand> --help` for each subcommand's flags.

## Output

Everything lands under `output/`, split by `<task>/<region>/<platform>/`:

| Directory | Contents |
| --- | --- |
| `output/Manifest` | manifest and patch manifest |
| `output/Metadata` | downloaded patch resources, the cross-run cache |
| `output/Unpack` | merged unpacked files |
| `output/Uncensor/` | uncensor output (`Manifest` / `Metadata` / `Resource` / `Default`) |
| `output/Launcher` | launcher resources and manifest |

Files whose hash already matches are skipped and logged as `cached:`; set `IS_OVERWRITE_RESOURCE` to force a re-download.

## Development

```bash
cargo test                 # 185 tests: inline unit tests in src/** plus tests/ integration
cargo clippy --all-targets
cargo fmt --all -- --check
```

`tests/fixtures/` holds real server data (a plaintext manifest, a bsdiff patch) so a format change breaks the build. Synthetic data checks the algorithm, real fixtures check the format — you need both.
