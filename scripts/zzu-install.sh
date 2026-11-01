#!/usr/bin/env bash
# zzu-install.sh —— 搬文件 + 打印后续命令
#
# 只做三件事：
#   1. 把 target/release/zzu_wifi_login  搬到 /opt/zzu_wifi_login/zzu_wifi_login
#   2. 把 systemd/zzu-wifi-login.service 搬到 /opt/zzu_wifi_login/zzu-wifi-login.service
#   3. 打印接下来手动要执行的命令
#
# 不做：不编译、不改 unit 内容、不 enable、不 start、不碰凭据
#
# 用法: sudo ./scripts/zzu-install.sh

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INSTALL_DIR="${ZZU_INSTALL_DIR:-/opt/zzu_wifi_login}"
BIN_NAME="zzu_wifi_login"
SERVICE_NAME="zzu-wifi-login.service"

BINARY="${ZZU_BINARY:-$REPO_DIR/target/release/$BIN_NAME}"
UNIT_SRC="$REPO_DIR/systemd/$SERVICE_NAME"

say()  { printf '%s\n' "$*"; }
die()  { printf '错误: %s\n' "$*" >&2; exit 1; }

if [ "$(id -u)" -ne 0 ]; then
	die "需要 root 运行。请执行:  sudo $0"
fi

[ -f "$BINARY" ] || die "找不到二进制: $BINARY
先执行:  cargo build --release"
[ -x "$BINARY" ] || die "不是可执行文件: $BINARY"

# unit 默认取 systemd/zzu-wifi-login.service；名字不对但只有一个 *.service 时自动采用
if [ ! -f "$UNIT_SRC" ]; then
	found="$(find "$REPO_DIR/systemd" -maxdepth 1 -name '*.service' 2>/dev/null | sort)"
	count="$(printf '%s' "$found" | grep -c . || true)"
	if [ "$count" -eq 1 ]; then
		UNIT_SRC="$found"
		say "提示: $REPO_DIR/systemd 里没有 $SERVICE_NAME，改用 $(basename "$UNIT_SRC")"
		say "      安装后仍命名为 $SERVICE_NAME"
	elif [ "$count" -gt 1 ]; then
		die "$REPO_DIR/systemd 里有多个 .service，无法判断装哪个：
$found
请把它们之一改名为 $SERVICE_NAME"
	else
		die "找不到 unit 文件: $REPO_DIR/systemd/$SERVICE_NAME"
	fi
fi

install -d -m 0755 "$INSTALL_DIR"
install -m 0755 "$BINARY"   "$INSTALL_DIR/$BIN_NAME"
install -m 0644 "$UNIT_SRC" "$INSTALL_DIR/$SERVICE_NAME"

say "已装:"
say "  $INSTALL_DIR/$BIN_NAME          (0755)"
say "  $INSTALL_DIR/$SERVICE_NAME     (0644)"
say ""
say "接下来手动执行:"
say ""
say "  # 1. 链进 systemd（unit 原件留在 /opt，/etc 下只放链接）"
say "  sudo ln -sfn $INSTALL_DIR/$SERVICE_NAME /etc/systemd/system/$SERVICE_NAME"
say ""
say "  # 2. 让 systemd 读到新 unit"
say "  sudo systemctl daemon-reload"
say ""
say "  # 3. 先手工验证一次（前台跑，Ctrl-C 停）"
say "  sudo systemctl start $SERVICE_NAME"
say "  systemctl status $SERVICE_NAME --no-pager"
say "  journalctl -u $SERVICE_NAME -n 30 --no-pager"
say ""
say "  # 4. 没问题再开机自启"
say "  sudo systemctl enable $SERVICE_NAME"
say ""
say "改动 unit 后的固定动作:  sudo systemctl daemon-reload && sudo systemctl restart $SERVICE_NAME"