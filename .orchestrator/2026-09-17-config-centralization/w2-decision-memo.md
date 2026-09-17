# W2 决策备忘录（等人裁决项的准备链）

## 裁决点：FTP 服务配置与静态对端簿要不要开 GUI 面（W2 后半）

**选项 A：W2 纳入**——设置页·服务区新增 FTP 卡（root/账号表/authz 开关，读写 ftp.json）
+ 网络区新增静态对端卡（static-peers.json CRUD）。
影响：账号表属凭据面（需 password 控件 + 不回显明文），静态对端属网络暴露面
（可被用于主动连接任意地址）；两者都需要新的 tauri 命令（ftp_config_get/save、
static_peers_list/upsert/remove）→ cli-parity 登记与契约 §3 扩表。
回滚：独立提交可单独 revert；新命令增面不破坏旧面。

**选项 B：W2 只做入口归拢**（bootstrap/relayAddrs/authzDefaultRole 三处编辑入口进设置页，
纯前端 + 零新命令），FTP/static-peers 维持文件配置 + CLI 面。
影响：最小改动、零新攻击面；代价=非 CLI 用户无法配置 FTP/静态对端。

**建议：B 先行（W2a），A 拆独立波（W2b）单独评审凭据与暴露面**——FTP 账号表涉及
凭据存储形态（明文 ftp.json 现状 vs 后端加密），static-peers 涉及主动连接暴露面，
值得独立设计而非搭车。

**回滚**：两选项均为纯增量功能提交，revert 即回文件配置现状。
