# ss-resource-fetcher

[![CI](https://github.com/MakoStar/ss-resource-fetcher/actions/workflows/ci.yml/badge.svg)](https://github.com/MakoStar/ss-resource-fetcher/actions/workflows/ci.yml)

[English](README.en.md) | 简体中文

《StellaSora》多区域资源抓取工具。流水线：拉取清单 → AES-128-CBC 解密 → protobuf 解码 → 提取补丁清单 → 下载资源 → bsdiff 合并，覆盖 CN / EN / JP / KR / TW 五个区域。

## 功能特性

- 三条独立链路：`client` 资源流水线、`launcher` 启动器包、`uncensor` 反和谐包
- 按波次（tier）下载与合并，补丁链前后依赖完整还原到最新版本
- 哈希校验 + 跨运行缓存，重复运行不重复下载与合并
- 平台感知：`DEFAULT_PLATFORM` 切换 win / and / ios，产物按平台分目录
- 命令行可覆盖任意配置项，无需改文件

## 安装

需要 Rust 1.85+（edition 2024）。

```bash
git clone https://github.com/MakoStar/ss-resource-fetcher.git
cd ss-resource-fetcher
cargo build --release
```

可执行文件位于 `target/release/ResourceFetcher`，下文简写为 `ResourceFetcher`。

## 使用方法

```bash
ResourceFetcher client                 # 完整流水线：拉清单 → 下载 → 合并 → 版本 JSON
ResourceFetcher client -r              # 仅 dump 清单，不下载
ResourceFetcher launcher -a            # 下载 launcher 全部资源
ResourceFetcher uncensor -r CN         # 下载 CN 的反和谐资源包
```

不带参数运行只打印帮助。三个子命令各管一条独立链路：

| 子命令 | 说明 |
| --- | --- |
| `client` | 常规资源流水线（下载 + 合并 + 版本 JSON） |
| `launcher` | 下载 launcher 资源包 |
| `uncensor` | 下载反和谐资源包 |

`launcher` 的下载范围必须显式指定（`-a` / `-p` / `-f` / `-e` 四选一）；`uncensor` 支持区域、默认地址、自定义名单三种模式。

## 修改配置

默认使用内置默认值，**不写入任何文件**。修改配置有两种方式：

**临时覆盖** —— `-s 段.键=值`，仅本次运行生效，可重复传入：

```bash
ResourceFetcher client -s DEFAULT_REGION=JP
ResourceFetcher client -s DEFAULT_PLATFORM=and            # win / and / ios
ResourceFetcher client -s PATCH_TAGS=[u,n]                # 把整文件补丁也纳入下载
ResourceFetcher -d TASK.CLIENT.SERVERS.CN client          # 移除某个配置项
```

**持久化配置** —— 先生成配置文件再手改：

```bash
ResourceFetcher --gen_config        # 生成 ss-resource-fetcher.toml（覆盖已有）
ResourceFetcher --print_keys        # 列出全部键、类型与默认值
```

配置文件为 TOML，按 `GLOBAL` / `NETWORK` / `PATCH_UPDATE_TAG` / `TASK.CLIENT` / `TASK.LAUNCHER` / `TASK.UNCENSOR` 分段组织。数组用方括号（`-s 'FILES=[a,b]'`），重复传同一个键则追加；平台相关字段存储的是当前平台的真实值，切换 `DEFAULT_PLATFORM` 后下次加载自动跟随。

## 命令行参数

以 `--help` 为准：

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

各子命令的参数见 `ResourceFetcher <子命令> --help`。

## 输出

产物统一落在 `output/` 下，按 `<任务>/<区域>/<平台>/` 分层：

| 目录 | 内容 |
| --- | --- |
| `output/Manifest` | 清单与补丁清单 |
| `output/Metadata` | 下载的补丁资源，跨运行复用的缓存 |
| `output/Unpack` | 合并后的解包产物 |
| `output/Uncensor/` | 反和谐产物（`Manifest` / `Metadata` / `Resource` / `Default`） |
| `output/Launcher` | launcher 资源与清单 |

哈希一致的文件自动跳过，日志标记 `cached:`；需要强制重新下载时设 `IS_OVERWRITE_RESOURCE`。

## 开发

```bash
cargo test                 # 185 个测试：src/** 内联单元测试 + tests/ 集成测试
cargo clippy --all-targets
cargo fmt --all -- --check
```

`tests/fixtures/` 存放服务端真实数据（明文清单、bsdiff 补丁），用于锁住格式兼容性；合成数据验证算法逻辑，真实夹具验证格式，两者互补。
