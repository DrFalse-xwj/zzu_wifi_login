#!/usr/bin/env bash
# zzu-cred-setup.sh —— 管理 ZZU-WLAN 登录密码的加密凭据
#
# 三件事：
#   init    首次配置：缺 host key 就生成（先问），再加密密码
#   change  修改密码：重新加密，旧文件自动备份
#   verify  检查现有凭据能否解开（不显示内容）
#
# 必须用 root 运行：TPM 设备 (/dev/tpmrm0) 和 host key 都只有 root 能访问。

set -euo pipefail
umask 077

SCRIPT="${BASH_SOURCE[0]}"
CRED_NAME="${ZZU_CRED_NAME:-zzu-pass}"
CRED_FILE="${ZZU_CRED_FILE:-/etc/credstore.encrypted/zzu-pass.cred}"
HOST_KEY="${ZZU_HOST_KEY:-/var/lib/systemd/credential.secret}"
PASS=""

say() { printf '%s\n' "$*"; }
err() { printf '错误: %s\n' "$*" >&2; }
die() { err "$*"; exit 1; }

# 退出时清掉内存里的密码
cleanup() { PASS=""; }
trap cleanup EXIT

usage() {
	cat <<EOF
用法: sudo $SCRIPT [init|change|verify]

  init     首次配置：缺 host key 就生成（会先确认），然后加密密码
  change   修改密码：重新加密，旧凭据自动备份
  verify   检查现有凭据能否解开（不打印内容）

不带参数进入交互菜单。
EOF
}

need_root() {
	local hint="${1:-}"
	if [ "$(id -u)" -ne 0 ]; then
		err "需要 root 权限：TPM 设备和 host key 只有 root 能读"
		if [ -n "$hint" ]; then
			say "请这样运行:  sudo $SCRIPT $hint"
		else
			say "请这样运行:  sudo $SCRIPT"
		fi
		exit 1
	fi
}

need_tools() {
	command -v systemd-creds >/dev/null 2>&1 || die "找不到 systemd-creds"
	command -v systemd-analyze >/dev/null 2>&1 || die "找不到 systemd-analyze"
}

# 有 TPM 就绑 host+tpm2，没有就只用 host key
detect_key_mode() {
	if systemd-analyze has-tpm2 >/dev/null 2>&1; then
		printf 'host+tpm2'
	else
		printf 'host'
	fi
}

# 第 1 件事：host key 不存在时，问过用户再生成
ensure_host_key() {
	if [ -f "$HOST_KEY" ]; then
		say "host key 已存在: $HOST_KEY"
		return 0
	fi

	say ""
	say "这台机器还没有 systemd 的 host key："
	say "  $HOST_KEY"
	say ""
	say "它是解密的根：加密凭据时用它的一个副本参与加密，"
	say "解密时必须有它。它丢了，已加密的凭据就再也解不开。"
	say ""
	say "即将执行:  systemd-creds setup"
	say "这个命令只新建上面那一个文件，不改动任何现有配置。"
	say ""
	printf '现在生成吗？[y/N] '
	local ans
	IFS= read -r ans || ans=""
	case "$ans" in
		y | Y | yes | YES) ;;
		*) die "已取消，没有 host key 无法继续" ;;
	esac

	systemd-creds setup
	[ -f "$HOST_KEY" ] || die "setup 执行了，但 $HOST_KEY 仍不存在"
	say "已生成: $HOST_KEY"
}

# 读两次密码，确认非空且一致，结果放进 PASS
read_password() {
	local p1 p2
	printf '输入新密码（不回显）: '
	# IFS= 防止首尾空格被吃掉；-r 保留反斜杠；-s 不回显
	IFS= read -rs p1 || true
	say ""
	printf '再输一次: '
	IFS= read -rs p2 || true
	say ""

	[ -n "$p1" ] || die "密码不能为空"
	[ "$p1" = "$p2" ] || die "两次输入不一致"
	[ "${#p1}" -le 256 ] || die "密码太长（>256 字节），可能粘错了"

	PASS="$p1"
	p1=""
	p2=""
}

backup_existing() {
	local file="$1" bak
	if [ -f "$file" ]; then
		bak="${file}.bak.$(date +%Y%m%d-%H%M%S)"
		cp -a "$file" "$bak"
		say "旧凭据已备份: $bak"
	fi
}

# 加密：密码只走管道，不进命令行、不进 ps、不进 history
encrypt_to() {
	local file="$1" mode
	mode="$(detect_key_mode)"
	say "加密方式: --with-key=$mode"

	mkdir -p "$(dirname "$file")"
	chmod 700 "$(dirname "$file")"

	if ! printf '%s' "$PASS" | systemd-creds encrypt \
		--name="$CRED_NAME" \
		--with-key="$mode" \
		- "$file"; then
		die "加密失败。若是 TPM 报错，检查 systemd-analyze has-tpm2 的输出"
	fi

	chown root:root "$file"
	chmod 600 "$file"
	say "已写入: $file"
}

# 解密到 stdout。
# --name 不能省：systemd 默认拿“文件名”和密文里内嵌的名字比对，
# 我们的文件叫 zzu-pass.cred，内嵌名是 zzu-pass，不传 --name 会被拒绝
# （报 Embedded credential name does not match filename）。
# unit 里不受影响，LoadCredentialEncrypted= 用的是冒号左边的 ID。
decrypt_cred() {
	systemd-creds decrypt --name="$CRED_NAME" "$1" -
}

# 回读校验：比对哈希，不打印明文
verify_roundtrip() {
	local file="$1" a b
	a="$(printf '%s' "$PASS" | sha256sum | cut -d' ' -f1)"
	b="$(decrypt_cred "$file" | sha256sum | cut -d' ' -f1)"

	if [ "$a" != "$b" ]; then
		err "回读校验失败：解出来的内容和刚输入的密码不一致"
		err "常见原因：加密时混进了换行符（用了 echo 而不是 printf）"
		return 1
	fi
	say "回读校验通过"
}

cmd_init() {
	need_root "init"
	need_tools

	if [ -f "$CRED_FILE" ]; then
		say "凭据已存在: $CRED_FILE"
		say "要改密码请用:  sudo $SCRIPT change"
		return 0
	fi

	ensure_host_key
	read_password
	encrypt_to "$CRED_FILE"
	verify_roundtrip "$CRED_FILE"

	say ""
	say "完成。在 unit 的 [Service] 段加上这一行："
	say "    LoadCredentialEncrypted=$CRED_NAME:$CRED_FILE"
	say ""
	say "程序里从 \$CREDENTIALS_DIRECTORY/$CRED_NAME 读取即可。"
}

cmd_change() {
	need_root "change"
	need_tools

	[ -f "$CRED_FILE" ] || die "还没有凭据，先运行:  sudo $SCRIPT init"

	ensure_host_key
	read_password
	backup_existing "$CRED_FILE"
	encrypt_to "$CRED_FILE"
	verify_roundtrip "$CRED_FILE"

	say ""
	say "密码已更新。重启服务让它生效："
	say "    sudo systemctl restart zzu-wifi-login.service"
}

cmd_verify() {
	need_root "verify"
	need_tools

	[ -f "$CRED_FILE" ] || die "没有 $CRED_FILE"

	say "文件: $CRED_FILE"
	stat -c '权限: %a  属主: %U:%G  大小: %s 字节' "$CRED_FILE"

	local len last
	len="$(decrypt_cred "$CRED_FILE" | wc -c)"
	[ "$len" -gt 0 ] || die "解密结果为空"

	say "解密成功，长度 ${len} 字节（内容不显示）"

	last="$(decrypt_cred "$CRED_FILE" | tail -c 1 | od -An -tx1 | tr -d ' \n')"
	if [ "$last" = "0a" ]; then
		say ""
		say "警告：末尾有一个换行符 (0x0a)。"
		say "多半是当初用 echo 加密的，登录会因为多这一个字节失败。"
		say "用  sudo $SCRIPT change  重新加密即可。"
	fi
}

menu() {
	say "ZZU-WLAN 凭据管理"
	say "  1) 首次配置  (init)"
	say "  2) 修改密码  (change)"
	say "  3) 检查凭据  (verify)"
	printf '选择 [1-3]: '
	local c
	IFS= read -r c || c=""
	case "$c" in
		1) cmd_init ;;
		2) cmd_change ;;
		3) cmd_verify ;;
		*) die "无效选择" ;;
	esac
}

main() {
	case "${1:-}" in
		init) cmd_init ;;
		change) cmd_change ;;
		verify) cmd_verify ;;
		-h | --help | help) usage ;;
		"")
			need_root ""
			need_tools
			menu
			;;
		*)
			usage
			exit 1
			;;
	esac
}

main "$@"
