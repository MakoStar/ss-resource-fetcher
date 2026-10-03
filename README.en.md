# ss-resource-fetcher

English | [简体中文](README.md)

Multi-region resource fetcher for StellaSora. Pipeline: fetch manifest → decrypt → decode → extract patch manifest → download → merge patches.

## Quick start

The first run generates `ss-resource-fetcher.toml` in the working directory. Edit it and run again.

```bash
ResourceFetcher                  # full pipeline
ResourceFetcher run -r           # manifest record only
ResourceFetcher uncensor         # uncensor pack only
ResourceFetcher launcher -a      # every launcher resource
```

## Command overview

```
ResourceFetcher [GLOBAL] [SUBCOMMAND [OPTIONS]]
```

| Level | Command / flag | Description |
| --- | --- | --- |
| Global | `--comments` | Write comments into the generated config |
| Global | `--init` `[BOOL]` | Generate config: unset=if missing, `true`=regenerate, `false`=skip |
| Global | `-s` `--set` `<KEY=VALUE>` | Override a config item, repeatable, works with any subcommand |
| Subcommand | `run` (default) | Resource pipeline |
| Subcommand | `uncensor` | Download the uncensor pack (exclusive, skips the pipeline) |
| Subcommand | `launcher` | Download the launcher pack (exclusive, skips the pipeline) |

Global flags are position-free: `ResourceFetcher --comments run` and `ResourceFetcher run -s ...` both work.

## Every flag combination

### `run`

| Combination | Effect |
| --- | --- |
| `run` | Full pipeline (fetch → download → merge → version JSON) |
| `run -r` / `run --record` | Manifest record only, no download, no merge |

### `uncensor`

| Combination | Effect |
| --- | --- |
| `uncensor` | Region mode, uses `[UNCENSOR].SOURCE_REGION`, names from the third-party manifest |
| `uncensor -r CN` | Pick the region, overrides the configured source region |
| `uncensor -d` | Default URL mode, skips the official manifest, writes to `UncensorDefault` |
| `uncensor -c -p list.json` | Custom list, region taken from the JSON `region` field |
| `uncensor -c -p list.json -r CN` | Custom list plus explicit region |
| `uncensor -r CN -s SOURCE_REGION=CN` | Override the source region with `-s` (same as `-r`) |

Conflicts: `-r` × `-d`, `-c` × `-d`; `-c` requires `-p`.

```jsonc
// uncensor custom list.json
{ "region": "TW", "files": ["char_2d_14901.unity3d"] }
```

### `launcher`

Three independent dimensions: **region** × **scope (required)** × **layout (optional)**.

| Dimension | Flag | Values |
| --- | --- | --- |
| Region | `-r` `--region` `<REGION>` | CN / EN / JP / KR / TW, case-insensitive; unset = all regions |
| Scope | `-a` `--all` | every manifest file |
| Scope | `-p` `--pattern` `<REGEX>` | regex, repeatable |
| Scope | `-f` `--file` `<NAME>` | file names, repeatable |
| Scope | `-e` `--regex` | use configured `[LAUNCHER].PATTERNS` |
| Scope | `-e false` | use configured `[LAUNCHER].FILES` |
| Layout | `-k` `--keep` | save by manifest path, flattened by default |

| Combination | Effect |
| --- | --- |
| `launcher -a` | all regions × every file |
| `launcher -a -k` | everything, keeping the directory layout (a full client pull) |
| `launcher -r CN -a` | every file of CN |
| `launcher -p '^app\.info$'` | all regions × regex match |
| `launcher -r CN -p '^app\.info$' -p '^boot\.config$'` | CN × several regexes |
| `launcher -r CN -f app.info -f boot.config` | CN × listed files |
| `launcher -r CN -e` | CN × configured PATTERNS |
| `launcher -r CN -e false` | CN × configured FILES |
| `launcher -r CN -e false -s 'FILES=[a.dll,b.dat]'` | feed FILES inline with `-s` |
| `launcher -r CN -a -k` | CN × everything × keep layout |
| `launcher -r CN -k -p '^app\.info$'` | CN × regex × keep layout |

Rules:

- **The scope must be explicit**: with none of `-a` / `-p` / `-f` / `-e` it errors out; `-a` conflicts with `-p` and `-f`.
- `-e` may combine with `-p` (`-p` wins); `-e false` may combine with `-f` (`-f` wins).
- Priority: `-a` > `-f` > `-e false` > `-p` > `-e`.
- If `IS_USE_DEFAULT_REGION` filtered the region out, `-r` falls back to the built-in endpoint (logged as `built-in endpoint`);
  without `-r` only the surviving regions run — add `-s IS_USE_DEFAULT_REGION=false` for all of them.
- `-k`: flattened by default (`output/Launcher/CN/data.unity3d`); with `-k` the manifest path is kept (`output/Launcher/CN/xtlr_Data/data.unity3d`).
- `-f` matches by file name, or by path when it contains `/`; unknown entries are warned about.
- A single failed file (e.g. HTTP 403) is warned and skipped; the run ends with a failure count.

## Overriding config

`-s SECTION.KEY=VALUE` overrides any config item for this run only, and may be repeated.

| Form | Example |
| --- | --- |
| Full path | `-s FEATURE_FLAGS.IS_MERGE_PATCH=false` |
| Shorthand (section omitted) | `-s IS_DOWNLOAD_RESOURCE=false` |
| Bool | `-s IS_USE_DEFAULT_REGION=false` |
| Number | `-s REQUEST.TIMEOUT_SECS=60` |
| String (quotes optional) | `-s DEFAULT_REGION=CN`, `-s FILE_PATH.UNPACK_OUTPUT_DIR=./unpack` |
| Nested table | `-s SERVERS.CN.URL=https://example.com` |
| Array (brackets) | `-s 'LAUNCHER.FILES=[a.dll,b.dat]'` |
| Array (strict TOML) | `-s 'LAUNCHER.FILES=["a.dll","b.dat"]'` |
| Array (repeat to append) | `-s FILES=a.dll -s FILES=b.dat` |

- Shorthand lookup searches `FEATURE_FLAGS`, `FILE_PATH`, `FILE_NAME`, `REQUEST`, `EXTRACTOR`, `SERVER_ROUTE`, `LAUNCHER`.
- For list keys the first override **replaces** the value, repeating the same key **appends**; `[a,b]` gives several at once.
- An unknown key is a hard error: `unknown config key 'NOPE', use SECTION.KEY like FEATURE_FLAGS.IS_MERGE_PATCH`.
- Each override logs `config override: xxx`; overrides apply before region filtering and validation.

## Config file

| Combination | Effect |
| --- | --- |
| (default) | Generate when missing, otherwise read it |
| `--comments` | Write Chinese comments into the generated config |
| `--init` | Force regeneration, overwriting the existing file |
| `--init=false` | Do not generate; run with built-in defaults |
| `--comments --init` | Regenerate a commented config |

`--comments` only applies on the run that generates the file; otherwise it prints a hint.

## Output

| Directory | Contents |
| --- | --- |
| `output/Manifest` | manifests and patch manifests |
| `output/Metadata` | downloaded patch resources |
| `output/Unpack` | merged unpacked files |
| `output/Uncensor` | uncensor resources (region / custom list) |
| `output/UncensorDefault` | uncensor resources (default URL) |
| `output/Launcher` | launcher resources and manifest, per region |

Files whose hash already matches are skipped; force a re-download with `IS_OVERWRITE_RESOURCE`.
