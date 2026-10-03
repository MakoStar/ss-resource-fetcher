# ss-resource-fetcher

[English](README.en.md) | 简体中文

《StellaSora》多区域资源抓取工具。流程：拉清单 → 解密 → 解码 → 提取补丁清单 → 下载资源 → 合并补丁。

## 快速开始

首次运行会在程序目录生成 `ss-resource-fetcher.toml`，改完重跑即可。

```bash
ResourceFetcher                  # 完整流水线
ResourceFetcher run -r           # 只生成清单记录
ResourceFetcher uncensor         # 只下载反和谐资源包
ResourceFetcher launcher -a      # 下载 launcher 全部资源
```

## 命令总览

```
ResourceFetcher [全局选项] [子命令 [子命令选项]]
```

| 层级 | 命令 / 参数 | 说明 |
| --- | --- | --- |
| 全局 | `--comments` | 生成配置文件时写入注释（仅生成那次生效） |
| 全局 | `--init` `[BOOL]` | 是否生成配置文件：不传=缺失时生成，`true`=重新生成，`false`=不生成 |
| 全局 | `-s` `--set` `<KEY=VALUE>` | 覆盖配置项，可重复，任意子命令都能用 |
| 子命令 | `run`（不传时默认） | 常规资源流水线 |
| 子命令 | `uncensor` | 下载反和谐资源包（独占，不跑流水线） |
| 子命令 | `launcher` | 下载 launcher 资源包（独占，不跑流水线） |

全局选项写在哪都行：`ResourceFetcher --comments run`、`ResourceFetcher run -s ...` 等价。

## 全部参数组合

### `run`

| 组合 | 效果 |
| --- | --- |
| `run` | 完整流水线（拉清单 → 下载 → 合并 → 版本 JSON） |
| `run -r` / `run --record` | 只生成清单记录，不下载不合并 |

### `uncensor`

| 组合 | 效果 |
| --- | --- |
| `uncensor` | 区域模式，用 `[UNCENSOR].SOURCE_REGION`，名单来自第三方清单 |
| `uncensor -r CN` | 指定区域，覆盖配置里的来源区域 |
| `uncensor -d` | 默认地址模式，不碰官方清单，直下到 `UncensorDefault` |
| `uncensor -c -p list.json` | 自定义名单，区域取 JSON 里的 `region` |
| `uncensor -c -p list.json -r CN` | 自定义名单并指定区域 |
| `uncensor -r CN -s SOURCE_REGION=CN` | 用 `-s` 覆盖配置来源（等价 `-r`） |

互斥：`-r` × `-d`、`-c` × `-d`；`-c` 必须配 `-p`。

```jsonc
// uncensor custom list.json
{ "region": "TW", "files": ["char_2d_14901.unity3d"] }
```

### `launcher`

三个维度自由组合：**区域** × **下载范围（必选）** × **保存方式（可选）**。

| 维度 | 参数 | 取值 |
| --- | --- | --- |
| 区域 | `-r` `--region` `<REGION>` | CN / EN / JP / KR / TW，不区分大小写；不传=全部区域 |
| 范围 | `-a` `--all` | 清单全部文件 |
| 范围 | `-p` `--pattern` `<REGEX>` | 正则，可重复 |
| 范围 | `-f` `--file` `<NAME>` | 文件名列表，可重复 |
| 范围 | `-e` `--regex` | 用配置 `[LAUNCHER].PATTERNS` |
| 范围 | `-e false` | 用配置 `[LAUNCHER].FILES` |
| 保存 | `-k` `--keep` | 按清单 path 建目录，默认摊平成文件名 |

| 组合 | 效果 |
| --- | --- |
| `launcher -a` | 全部区域 × 全部文件 |
| `launcher -a -k` | 全量并保持目录结构（等同拉回完整客户端） |
| `launcher -r CN -a` | 只下 CN 的全部文件 |
| `launcher -p '^app\.info$'` | 全部区域 × 正则命中 |
| `launcher -r CN -p '^app\.info$' -p '^boot\.config$'` | CN × 多个正则 |
| `launcher -r CN -f app.info -f boot.config` | CN × 指定文件 |
| `launcher -r CN -e` | CN × 配置 PATTERNS |
| `launcher -r CN -e false` | CN × 配置 FILES |
| `launcher -r CN -e false -s 'FILES=[a.dll,b.dat]'` | 用 `-s` 临时给 FILES |
| `launcher -r CN -a -k` | CN × 全量 × 保持目录 |
| `launcher -r CN -k -p '^app\.info$'` | CN × 正则 × 保持目录 |

约束：

- **下载范围必须显式指定**：`-a` / `-p` / `-f` / `-e` 都不给直接报错；`-a` 与 `-p`、`-f` 互斥。
- `-e` 与 `-p` 可共存（`-p` 优先）；`-e false` 与 `-f` 可共存（`-f` 优先）。
- 优先级：`-a` > `-f` > `-e false` > `-p` > `-e`。
- 区域被 `IS_USE_DEFAULT_REGION` 过滤掉时，`-r` 会回退内置端点（日志标 `built-in endpoint`）；
  不传 `-r` 则只跑过滤后剩下的区域，想跑全部加 `-s IS_USE_DEFAULT_REGION=false`。
- `-f` 传纯文件名按文件名匹配，带 `/` 时按 path 匹配；清单里找不到会告警。
- 单文件失败（如 403）只告警跳过，结束汇总。

## 命令行覆盖配置项

`-s 段.键=值` 临时覆盖任意配置项，只在本次运行生效，可重复传入。

| 写法 | 示例 |
| --- | --- |
| 完整路径 | `-s FEATURE_FLAGS.IS_MERGE_PATCH=false` |
| 简写（省段名） | `-s IS_DOWNLOAD_RESOURCE=false` |
| 布尔 | `-s IS_USE_DEFAULT_REGION=false` |
| 数值 | `-s REQUEST.TIMEOUT_SECS=60` |
| 字符串（可不加引号） | `-s DEFAULT_REGION=CN`、`-s FILE_PATH.UNPACK_OUTPUT_DIR=./unpack` |
| 嵌套表 | `-s SERVERS.CN.URL=https://example.com` |
| 数组（方括号） | `-s 'LAUNCHER.FILES=[a.dll,b.dat]'` |
| 数组（TOML 严格写法） | `-s 'LAUNCHER.FILES=["a.dll","b.dat"]'` |
| 数组（重复追加） | `-s FILES=a.dll -s FILES=b.dat` |

- 简写会在 `FEATURE_FLAGS` / `FILE_PATH` / `FILE_NAME` / `REQUEST` / `EXTRACTOR` / `SERVER_ROUTE` / `LAUNCHER` 里查。
- 列表项：第一次覆盖**替换**原值，重复传同一个键则**追加**；`[a,b]` 一次给多个。
- 键写错直接报错：`unknown config key 'NOPE', use SECTION.KEY like FEATURE_FLAGS.IS_MERGE_PATCH`。
- 每次覆盖打一条 `config override: xxx`；覆盖在区域过滤与校验之前生效。

## 配置项文件

| 组合 | 效果 |
| --- | --- |
| （默认） | 配置文件缺失时生成，存在则直接读 |
| `--comments` | 生成配置文件时写入中文注释 |
| `--init` | 强制重新生成并覆盖已有文件 |
| `--init=false` | 不生成，用内置默认值运行 |
| `--comments --init` | 重新生成一份带注释的配置 |

`--comments` 只在生成那一次生效；配置已存在时会提示。

## 输出目录

| 目录 | 内容 |
| --- | --- |
| `output/Manifest` | 清单与补丁清单 |
| `output/Metadata` | 下载的补丁资源 |
| `output/Unpack` | 合并后的解包产物 |
| `output/Uncensor` | 反和谐资源（区域 / 自定义名单） |
| `output/UncensorDefault` | 反和谐资源（默认地址模式） |
| `output/Launcher` | launcher 资源与清单，按区域分目录 |

哈希一致的文件自动跳过，可用 `IS_OVERWRITE_RESOURCE` 强制重下。
