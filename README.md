# ss-resource-fetcher

《StellaSora》多区域资源抓取工具。流程：拉取清单 → 解密 → 解码 → 提取补丁清单 → 下载资源 → 合并补丁。

## 快速开始

首次运行会在程序目录自动生成配置文件 `ss-resource-fetcher.toml`，按需修改后重新运行即可。

```bash
ResourceFetcher          # 跑完整流水线
ResourceFetcher -g       # 只生成清单记录
ResourceFetcher -u       # 只下载反和谐资源包
```

## 参数

| 参数 | 说明 |
| --- | --- |
| `-g` `--generate-manifest-record` | 生成清单记录 |
| `-u` `--download-uncensor-pack` | 只下载反和谐资源包（独占任务） |
| `-r` `--uncensor-region` `<REGION>` | 反和谐下载所用区域，覆盖配置里的来源区域 |
| `-U` `--uncensor-default-url` | 直接用反和谐默认地址下载 |
| `-c` `--uncensor-custom-files` | 使用自定义反和谐文件列表 |
| `--uncensor-custom-files-path` `<PATH>` | 自定义文件列表 JSON 路径 |
| `-h` `--help` / `-V` `--version` | 查看帮助 / 版本 |

## 下载反和谐资源包

反和谐相关配置写在配置文件的 `[UNCENSOR]` 段。
`-u` 是独占任务：只下载资源包就返回，不跑常规流水线，也不受功能开关影响。
`-r` `-U` `-c` 只有配合 `-u` 时才生效。

```bash
ResourceFetcher -u                                   # 用配置里的来源区域，名单来自第三方清单
ResourceFetcher -u -r TW                             # 指定区域
ResourceFetcher -u -U                                # 用默认地址直接下载，不匹配官方清单
ResourceFetcher -u -c --uncensor-custom-files-path list.json       # 用自定义名单
ResourceFetcher -u -c --uncensor-custom-files-path list.json -r JP # 自定义名单并指定区域
```

自定义名单 JSON 格式，`region` 省略时用 `-r` 传入的区域：

```json
{ "region": "TW", "files": ["char_2d_14901.unity3d", "char_2d_14902.unity3d"] }
```

约束：`-r` 与 `-U` 互斥，`-c` 与 `-U` 互斥，`-c` 必须搭配 `--uncensor-custom-files-path`。

## 输出目录

| 目录 | 内容 |
| --- | --- |
| `output/Manifest` | 清单与补丁清单 |
| `output/Metadata` | 下载的补丁资源 |
| `output/Unpack` | 合并后的解包产物 |
| `output/Uncensor` | 反和谐资源（区域模式与自定义名单） |
| `output/UncensorDefault` | 反和谐资源（默认地址模式） |

已存在且哈希一致的文件会自动跳过，可用配置里的 `IS_OVERWRITE_RESOURCE` 强制重新下载。
