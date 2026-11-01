#!/usr/bin/env bash
# zzu-config-setup.sh —— 生成 ZZU-WLAN 登录程序的初始配置
#
# 只写非机密内容（学号、运营商、服务器地址、SSID、间隔）。
# 密码不在这里，用 zzu-cred-setup.sh 单独处理。
#
# 已存在旧配置时会先展示内容、征得同意再删除（并留一份 .bak）。

set -euo pipefail
umask 077

SCRIPT="${BASH_SOURCE[0]}"
case "$SCRIPT" in /*) ;; *) SCRIPT="$PWD/$SCRIPT" ;; esac
CONFIG_DIR="${ZZU_CONFIG_DIR:-/etc/zzu-wifi-login}"
CONFIG_FILE="${ZZU_CONFIG_FILE:-$CONFIG_DIR/config}"

DEFAULT_PORTAL="172.16.4.14"
DEFAULT_SSID="ZZU-WLAN"
DEFAULT_INTERVAL="30"

say() { printf '%s\n' "$*"; }
err() { printf '错误: %s\n' "$*" >&2; }
die() { err "$*"; exit 1; }

usage() {
	cat <<EOF
用法: sudo $SCRIPT

生成 $CONFIG_FILE。
已存在时会先显示内容并询问是否删除重配。

环境变量可覆盖路径（调试用）：
  ZZU_CONFIG_DIR   默认 /etc/zzu-wifi-login
  ZZU_CONFIG_FILE  默认 \$ZZU_CONFIG_DIR/config
EOF
}

need_root() {
	if [ "$(id -u)" -ne 0 ]; then
		err "需要 root 权限：要写 $CONFIG_FILE"
		say "请这样运行:  sudo $SCRIPT"
		exit 1
	fi
}

is_digits() {
	case "$1" in
		"" | *[!0-9]*) return 1 ;;
		*) return 0 ;;
	esac
}

is_ipv4() {
	local IFS=.
	# shellcheck disable=SC2086
	set -- $1
	[ $# -eq 4 ] || return 1
	local p
	for p in "$@"; do
		is_digits "$p" || return 1
		[ "$p" -le 255 ] || return 1
	done
	return 0
}

# 旧配置：先展示，再问，再备份后删除
handle_existing() {
	[ -f "$CONFIG_FILE" ] || return 0

	say "已存在配置: $CONFIG_FILE"
	say "----------------------------------------"
	sed 's/^/  /' "$CONFIG_FILE"
	say "----------------------------------------"
	say ""
	say "重新配置会先把这个文件删掉（它会被改名成 .bak 保留一份）。"
	printf '继续吗？[y/N] '
	local ans
	IFS= read -r ans || ans=""
	case "$ans" in
		y | Y | yes | YES) ;;
		*)
			say "已取消，什么都没改。"
			exit 0
			;;
	esac

	local bak="${CONFIG_FILE}.bak.$(date +%Y%m%d-%H%M%S)"
	mv "$CONFIG_FILE" "$bak"
	say "旧配置已移到: $bak"
	say ""
}

# 学号：6-20 位数字
ask_account() {
	local v
	while true; do
		printf '学号: '
		IFS= read -r v || v=""
		v="$(printf '%s' "$v" | tr -d '[:space:]')"

		if ! is_digits "$v"; then
			err "只能填数字"
			continue
		fi
		if [ "${#v}" -lt 6 ] || [ "${#v}" -gt 20 ]; then
			err "长度应在 6-20 位之间"
			continue
		fi
		ACCOUNT="$v"
		return 0
	done
}

# 运营商后缀：写死列表，不做自动探测
ask_carrier() {
	local c
	say "运营商后缀:"
	say "  1) 无（校园网直连）"
	say "  2) cmcc     中国移动"
	say "  3) unicom   中国联通"
	say "  4) telecom  中国电信"
	say "  5) zzuplan  校园套餐"
	while true; do
		printf '选择 [1-5，回车=1]: '
		IFS= read -r c || c=""
		case "$c" in
			"" | 1) CARRIER=""; return 0 ;;
			2) CARRIER="cmcc"; return 0 ;;
			3) CARRIER="unicom"; return 0 ;;
			4) CARRIER="telecom"; return 0 ;;
			5) CARRIER="zzuplan"; return 0 ;;
			*) err "请输入 1-5" ;;
		esac
	done
}

ask_portal() {
	local v
	while true; do
		printf '认证服务器地址 [回车=%s]: ' "$DEFAULT_PORTAL"
		IFS= read -r v || v=""
		v="$(printf '%s' "$v" | tr -d '[:space:]')"
		[ -z "$v" ] && v="$DEFAULT_PORTAL"

		if ! is_ipv4 "$v"; then
			err "格式不像 IPv4 地址"
			continue
		fi
		PORTAL="$v"
		return 0
	done
}

ask_ssid() {
	local v
	while true; do
		printf 'SSID [回车=%s]: ' "$DEFAULT_SSID"
		IFS= read -r v || v=""
		v="$(printf '%s' "$v" | tr -d '[:space:]')"
		[ -z "$v" ] && v="$DEFAULT_SSID"

		if [ "${#v}" -gt 32 ]; then
			err "SSID 最长 32 字节"
			continue
		fi
		SSID="$v"
		return 0
	done
}

ask_interval() {
	local v
	while true; do
		printf '检查间隔秒数 [回车=%s]: ' "$DEFAULT_INTERVAL"
		IFS= read -r v || v=""
		v="$(printf '%s' "$v" | tr -d '[:space:]')"
		[ -z "$v" ] && v="$DEFAULT_INTERVAL"

		if ! is_digits "$v"; then
			err "只能填数字"
			continue
		fi
		if [ "$v" -lt 5 ] || [ "$v" -gt 3600 ]; then
			err "建议范围 5-3600 秒"
			continue
		fi
		INTERVAL="$v"
		return 0
	done
}

write_config() {
	mkdir -p "$CONFIG_DIR"
	chmod 700 "$CONFIG_DIR"

	cat >"$CONFIG_FILE" <<EOF
# 由 zzu-config-setup.sh 生成于 $(date '+%Y-%m-%d %H:%M')
# 修改后需要 systemctl restart zzu-wifi-login.service
account=$ACCOUNT
carrier=$CARRIER
portal=$PORTAL
ssid=$SSID
interval_secs=$INTERVAL
EOF

	chown root:root "$CONFIG_FILE"
	chmod 640 "$CONFIG_FILE"
}

main() {
	case "${1:-}" in
		-h | --help | help)
			usage
			exit 0
			;;
		"") ;;
		*)
			usage
			exit 1
			;;
	esac

	need_root
	handle_existing

	ACCOUNT=""
	CARRIER=""
	PORTAL=""
	SSID=""
	INTERVAL=""

	ask_account
	ask_carrier
	ask_portal
	ask_ssid
	ask_interval

	write_config

	say ""
	say "已写入: $CONFIG_FILE"
	say "----------------------------------------"
	sed 's/^/  /' "$CONFIG_FILE"
	say "----------------------------------------"
	say ""
	if [ -n "$CARRIER" ]; then
		say "登录时会使用账号: $ACCOUNT@$CARRIER"
	else
		say "登录时会使用账号: $ACCOUNT（无后缀）"
	fi
	say ""
	say "下一步：配置密码"
	say "    sudo $(dirname "$SCRIPT")/zzu-cred-setup.sh init"
}

main "$@"
