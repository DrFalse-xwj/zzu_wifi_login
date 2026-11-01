# zzu_wifi_login

郑州大学 `ZZU-WLAN`（Dr.COM ePortal）自动登录 / 掉线重连，Rust 编写，由 systemd 守护，密码用 systemd 加密凭据保存。

## 要求

Linux 主机且 systemd ≥ 250（编译另需 Rust ≥ 1.85）；无其它运行时依赖，TPM 2.0 可选。

## ePortal 是什么

连上 `ZZU-WLAN` 后还不能直接上网：网关会把流量拦到认证页面，要输入学号密码才放行。本程序做的是同一件事——直接用学号密码调用 ePortal 的 HTTP 接口完成认证，并定时检查，掉线自动重登。

## 认证服务器地址

目前已知两个：

- `172.16.4.14` —— 配置里默认写死的地址，校园网一般用它
- `172.16.0.114` —— 另一个人群/区域会遇到的地址

如果连上后打不开网页、或者程序反复报 `web setup failed`，把配置里的 `portal=` 改成 `172.16.0.114` 再重启服务试试。

## 安装

```bash
git clone git@github.com:DrFalse-xwj/zzu_wifi_login.git
cd zzu_wifi_login
cargo build --release

# 1. 学号 / 运营商 / 认证服务器（生成 /etc/zzu-wifi-login/config）
sudo ./scripts/zzu-config-setup.sh

# 2. 加密保存密码（写 /etc/credstore.encrypted/zzu-pass.cred）
sudo ./scripts/zzu-cred-setup.sh init

# 3. 装二进制和 unit，然后按它打印的命令执行
sudo ./scripts/zzu-install.sh
```

第 3 步脚本只负责搬文件，接着手动执行它提示的几条：

```bash
sudo ln -sfn /opt/zzu_wifi_login/zzu-wifi-login.service /etc/systemd/system/zzu-wifi-login.service
sudo systemctl daemon-reload
sudo systemctl start zzu-wifi-login.service
sudo systemctl enable zzu-wifi-login.service
```

## 常用命令

```bash
systemctl status zzu-wifi-login.service --no-pager   # 状态
journalctl -u zzu-wifi-login.service -f              # 实时日志
sudo ./scripts/zzu-cred-setup.sh change              # 改密码
sudo ./scripts/zzu-cred-setup.sh verify              # 检查凭据能否解开
ZZU_PASS='密码' sudo -E ./target/release/zzu_wifi_login --once   # 手动跑一轮
```

改了配置或 unit 之后统一用：

```bash
sudo systemctl daemon-reload && sudo systemctl restart zzu-wifi-login.service
```

## 许可

MIT
