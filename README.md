# zzu_wifi_login

郑州大学 `ZZU-WLAN`（Dr.COM ePortal）校园网自动登录 / 断线重连守护程序。

用 Rust 写的小程序，不依赖任何第三方 crate（只用了标准库），密码借助 systemd 的加密凭据机制（`systemd-creds`）落盘，不存明文。开机后由 systemd 拉起，定时检查在线状态，掉线就自动用学号密码重新认证。

---

## 一、工作原理

1. 用 `ip route get <认证服务器>` 拿到真正出网的网卡名和本机 IPv4 地址；
2. 从 `/sys/class/net/<网卡>/address` 读 MAC；
3. 向认证服务器 `GET /drcom/chkstatus` 判断是否已在线；
4. 不在线就 `GET /eportal/portal/login`，账号拼成 `,0,<学号>@<运营商后缀>`，密码做 Base64 后 URL 编码，带上 IP / MAC / AC 地址（`172.16.4.252`）/ `jsVersion=4.2.2`；
5. 解析返回的 JSON 判断结果；然后 sleep `interval_secs` 秒，回到第 1 步。

全程只用 `std::net::TcpStream` 手写 HTTP/1.1 请求，所以**不需要 curl、openssl、reqwest**，也不存在证书或 TLS 依赖。

---

## 二、硬件要求

| 项目 | 要求 |
| --- | --- |
| 主机 | 任意能长期开机的 Linux 机器（x86_64 / aarch64 均可），可以是笔记本、小主机、树莓派 |
| 网卡 | 能连上 `ZZU-WLAN` 的无线网卡（或通过网关能到 `172.16.4.14` / `172.16.4.252`） |
| TPM 2.0 | **可选，推荐**。有 TPM 时密码绑 `host+tpm2` 加密，拔盘换机也解不开；没有 TPM 就退化为只用 host key 加密 |
| 磁盘 | 编译需要约 300 MB（仅 `target/`），运行时几乎不占空间 |
| 内存 | 只要几十 MB，任意配置 |

> 没有 TPM 的机器完全可用，只是密钥保护弱一档（密钥在 `/var/lib/systemd/credential.secret`，能读到该文件就等于能解密，所以务必保证 root 权限安全）。

---

## 三、软件要求

| 项目 | 要求 | 说明 |
| --- | --- | --- |
| 操作系统 | Linux | 依赖 systemd，**不支持** Windows / macOS / OpenWrt 等无 systemd 环境 |
| systemd | ≥ 250 | 需要 `systemd-creds`、`LoadCredentialEncrypted=`。EL9 / Debian 12 / Ubuntu 22.04 及以上都满足 |
| Rust | ≥ 1.85（edition 2024） | 仅编译时需要；运行不需要 Rust 环境 |
| iproute2 | `ip` 命令 | 用来查路由和网卡 |
| 权限 | root | 读 `/etc/credstore.encrypted/`、由 systemd 解密凭据、监听不需要特权端口 |
| 其他 | 无 | 不需要 curl / openssl / python / gcc（crate 已编译进二进制） |

检查环境：

```bash
systemctl --version | head -1          # 期望 >= 250
systemd-analyze has-tpm2 && echo "有 TPM2" || echo "无 TPM2（仍可用）"
cargo --version                        # 编译时才需要
ip -V
```

---

## 四、编译

```bash
git clone git@github.com:DrFalse-xwj/zzu_wifi_login.git
cd zzu_wifi_login
cargo build --release
# 产物：target/release/zzu_wifi_login
```

跑一遍测试（配置解析、Base64、响应解析都有单测）：

```bash
cargo test
```

---

## 五、使用（四步）

全部命令都需要 root。脚本会自己提示，不要跳过顺序。

### 第 1 步：写配置（学号、运营商、认证服务器）

```bash
sudo ./scripts/zzu-config-setup.sh
```

交互式询问：

| 提示 | 说明 |
| --- | --- |
| 学号 | 6–20 位纯数字 |
| 运营商后缀 | `1` 无后缀 / `2` cmcc 移动 / `3` unicom 联通 / `4` telecom 电信 / `5` zzuplan 校园套餐 |
| 认证服务器地址 | 回车用默认 `172.16.4.14` |
| SSID | 回车用默认 `ZZU-WLAN`（程序不主动连 WiFi，仅作记录） |
| 检查间隔秒数 | 回车默认 `30`，允许 5–3600 |

生成 `/etc/zzu-wifi-login/config`（权限 `640`，目录 `700`）：

```ini
# 由 zzu-config-setup.sh 生成于 2026-11-01 16:50
account=202600000001
carrier=cmcc
portal=172.16.4.14
ssid=ZZU-WLAN
interval_secs=30
```

字段含义：

- `account`：学号，**必填**
- `carrier`：运营商后缀，留空表示校园网直连；登录时拼成 `学号@后缀`
- `portal`：认证服务器 IP，默认 `172.16.4.14`
- `ssid`：只作记录，代码默认 `ZZU-WLAN`
- `interval_secs`：轮询间隔；代码内默认 60，脚本写入 30

重复运行会先打印旧配置并确认，确认后把旧文件改名为 `config.bak.<时间戳>` 再重写。改完配置要重启服务。

### 第 2 步：加密保存密码

```bash
sudo ./scripts/zzu-cred-setup.sh init     # 首次
sudo ./scripts/zzu-cred-setup.sh change   # 改密码
sudo ./scripts/zzu-cred-setup.sh verify   # 检查能否解开（不显示明文）
```

`init` 会：

1. 检查 host key `/var/lib/systemd/credential.secret`，不存在**先征得你同意**再 `systemd-creds setup`；
2. 不回显地读两遍密码，校验非空且一致；
3. 用管道把密码交给 `systemd-creds encrypt`，写 `/etc/credstore.encrypted/zzu-pass.cred`（`600`，root）；
4. 解密回读、比对 sha256，确认没有多出换行符。

有 TPM 时加密方式为 `--with-key=host+tpm2`，否则 `--with-key=host`。

> 密码只走管道，不进命令行参数、不进 `ps`、不进 shell history。
> **不要**用 `echo 密码 | systemd-creds encrypt ...`——那会多一个换行字节，登录会一直失败。`verify` 会专门检查这个。

### 第 3 步：装二进制 + unit

```bash
sudo ./scripts/zzu-install.sh
```

它只做三件事：把 `target/release/zzu_wifi_login` 装到 `/opt/zzu_wifi_login/`、把 unit 文件放过去、打印接下来要手动敲的命令。**不编译、不 enable、不 start、不碰凭据。**

按提示执行（脚本会把命令原样打出来）：

```bash
sudo ln -sfn /opt/zzu_wifi_login/zzu-wifi-login.service /etc/systemd/system/zzu-wifi-login.service
sudo systemctl daemon-reload
sudo systemctl start zzu-wifi-login.service
systemctl status zzu-wifi-login.service --no-pager
journalctl -u zzu-wifi-login.service -n 30 --no-pager
```

确认日志里出现 `already online` 或 `login successful` 之后，再设开机自启：

```bash
sudo systemctl enable zzu-wifi-login.service
```

### 第 4 步：手动验证一次（不装服务也能用）

```bash
ZZU_PASS='你的密码' sudo -E ./target/release/zzu_wifi_login --once
```

`--once` 表示执行一轮就退出（适合排查）。前台直接跑时，密码可以从环境变量 `ZZU_PASS` 读；作为服务跑时，密码由 systemd 解密后以文件形式放在 `$CREDENTIALS_DIRECTORY/zzu-pass`，程序优先读它。

无参数运行则进入守护循环，每 `interval_secs` 秒检查一次，掉线自动重登。

典型输出：

```
account  = 202600000001
carrier  = cmcc
portal   = 172.16.4.14
ssid     = ZZU-WLAN
interval = 30 秒
iface    = wlan0
src      = 10.62.13.77
mac      = aabbccddeeff
password = 12 字节（不打印内容）
结果: 登录成功
```

---

## 六、目录结构

```
.
├── Cargo.toml
├── Cargo.lock
├── src/
│   ├── main.rs        # 入口：读取配置 → 单次或守护循环
│   ├── get_config.rs  # /etc/zzu-wifi-login/config 解析（含单测）
│   ├── info_get.rs    # ip route get 解析网卡/IP，读 MAC
│   ├── key_get.rs     # 从 $CREDENTIALS_DIRECTORY 或 ZZU_PASS 取密码
│   └── login.rs       # 手写 HTTP、Base64/URL 编码、chkstatus、登录、回包解析
├── scripts/
│   ├── zzu-config-setup.sh   # 生成配置（非机密部分）
│   ├── zzu-cred-setup.sh     # init / change / verify 加密凭据
│   └── zzu-install.sh        # 装二进制与 unit，打印后续命令
└── systemd/
    └── zzu_wifi_login.service
```

unit 关键行：

```ini
[Service]
Type=simple
ExecStart=/opt/zzu_wifi_login/zzu_wifi_login
LoadCredentialEncrypted=zzu-pass:/etc/credstore.encrypted/zzu-pass.cred
Restart=on-failure
RestartSec=30
NoNewPrivileges=yes
```

---

## 七、常见问题

**`fail to read config file`**
还没生成配置，跑第 1 步。

**`Can't load password`**
服务外运行且没设 `ZZU_PASS`；或 `/etc/credstore.encrypted/zzu-pass.cred` 不存在。先 `sudo ./scripts/zzu-cred-setup.sh init`，再用 `verify` 确认能解密。

**`web setup failed` / `ip route get` 报错**
当前没有到认证服务器的路由——先连上 `ZZU-WLAN`（或处于需要认证的校园网环境中）。

**一直登录失败但密码没错**
先 `sudo ./scripts/zzu-cred-setup.sh verify`，若提示末尾有 `0x0a`，说明密码里混了换行，用 `change` 重新加密。
其次确认 `carrier` 选对（办的是移动就选 cmcc，选错等于账号不存在）。

**改了配置/unit 不生效**
```bash
sudo systemctl daemon-reload && sudo systemctl restart zzu-wifi-login.service
```

**想看实时日志**
```bash
journalctl -u zzu-wifi-login.service -f
```

**卸载**
```bash
sudo systemctl disable --now zzu-wifi-login.service
sudo rm -f /etc/systemd/system/zzu-wifi-login.service
sudo systemctl daemon-reload
sudo rm -rf /opt/zzu_wifi_login /etc/zzu-wifi-login
# 凭据与 host key 按需删除（host key 若还有别的服务在用就不要删）
sudo rm -f /etc/credstore.encrypted/zzu-pass.cred
```

---

## 八、安全说明

- 磁盘上**没有明文密码**，只有 `/etc/credstore.encrypted/zzu-pass.cred`（`600`，root）。
- 解密依赖 `/var/lib/systemd/credential.secret`（host key）。**这个文件丢了，凭据就永久解不开**，只能 `init` 重来（注意重新生成 host key 会让其它用旧 host key 加密的凭据一起失效）。
- 有 TPM2 时密文额外绑定本机 TPM，拷到别的机器解不开。
- 程序本身不需要明文配置文件里的密码，也不打印密码，只打印长度。
- 端口/协议为明文 HTTP（校园网 ePortal 本身就是 `http://172.16.4.x:801`），密码在传输层是 Base64，**不是加密**——这是校园网协议决定的，程序无法改变。

## 九、许可

仓库暂未附带 LICENSE 文件。
