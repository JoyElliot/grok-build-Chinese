#!/bin/sh

set -eu

# Do not trust a caller-controlled PATH for integrity and architecture checks.
CALLER_PATH=${PATH-}
PATH=/usr/bin:/bin:/usr/sbin:/sbin
export PATH

PROGRAM_NAME=Install-GrokZh.sh
WITH_COMPAT_ALIASES=0
STAGE_FILE=
LINK_SEQ=0

die() {
  printf '%s\n' "${PROGRAM_NAME}: $*" >&2
  exit 1
}

acquire_install_lock() {
  [ -x /usr/bin/perl ] || die "缺少系统 /usr/bin/perl，无法安全获取安装锁。"
  LOCK_FILE="$BIN_DIR/.grok-zh-install.lock"
  if [ -e "$LOCK_FILE" ] || [ -L "$LOCK_FILE" ]; then
    [ ! -L "$LOCK_FILE" ] && [ -f "$LOCK_FILE" ] || die "安装锁不是普通文件：$LOCK_FILE"
  fi
  exec 9<>"$LOCK_FILE" || die "无法打开安装锁：$LOCK_FILE"
  # Darwin flock belongs to the shared open-file description. The shell keeps
  # fd 9 open after this child exits, so the lock lasts until installer exit.
  # Never explicitly unlock or unlink this shared installer/updater lock.
  /usr/bin/env -i PATH="$PATH" /usr/bin/perl -MFcntl=:flock,:mode -MConfig -e '
    $Config{d_flock} eq "define" or die "native flock is unavailable\n";
    my $path = shift;
    open(my $lock, "+<&=9") or die "open lock descriptor: $!\n";
    my @fd = stat($lock);
    my @entry = lstat($path);
    @fd && @entry && S_ISREG($fd[2]) && S_ISREG($entry[2])
      && $fd[0] == $entry[0] && $fd[1] == $entry[1]
      && $fd[4] == $< && ($fd[2] & 07777) == 0600
      or die "unsafe install lock\n";
    flock($lock, LOCK_EX | LOCK_NB) or die "install lock is busy: $!\n";
    @entry = lstat($path);
    @entry && S_ISREG($entry[2]) && $fd[0] == $entry[0] && $fd[1] == $entry[1]
      or die "install lock changed during acquisition\n";
  ' "$LOCK_FILE" || die "无法获取安装锁；另一个 grok-zh 安装或自动更新可能正在进行，请稍后重试。"
}

usage() {
  cat <<'EOF'
用法：./Install-GrokZh.sh [--with-compat-aliases]

默认只安装 grok-zh 和 agent-zh。指定 --with-compat-aliases 后，还会在
同一个 ~/.grok/bin 目录中创建 grok -> grok-zh 和 agent -> agent-zh；
安装器不会修改 shell 配置，也不会写入 /usr/local/bin。
EOF
}

for arg in "$@"; do
  case "$arg" in
    --with-compat-aliases) WITH_COMPAT_ALIASES=1 ;;
    -h|--help) usage; exit 0 ;;
    *) die "未知参数：$arg" ;;
  esac
done

umask 077

[ "$(uname -s)" = Darwin ] || die "此安装器只支持 macOS。"
case "$(uname -m)" in
  arm64) PACKAGE_ARCH=aarch64; BINARY_ARCH=arm64 ;;
  x86_64) PACKAGE_ARCH=x86_64; BINARY_ARCH=x86_64 ;;
  *) die "此安装器只支持 macOS arm64 或 x86_64。" ;;
esac
[ ! -L "$0" ] || die "请直接运行软件包中的安装器，不要通过符号链接启动。"

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname "$0")" && pwd -P) || die "无法定位软件包目录。"
case "$SCRIPT_DIR" in
  *'
'*) die "软件包目录不能包含换行符。" ;;
esac

MANIFEST="$SCRIPT_DIR/SHA256SUMS.txt"
[ -f "$MANIFEST" ] && [ ! -L "$MANIFEST" ] || die "缺少可信的 SHA256SUMS.txt。"

EXPECTED_NAMES='BUILD-INFO.txt
INSTALL-MACOS.md
Install-GrokZh.sh
LICENSE-grok-build.txt
NOTICE-third-party.txt
SOURCE_REV
THIRD-PARTY-NOTICES-xai-grok-tools.md
THIRD-PARTY-NOTICES.txt
grok-zh'

seen_names=
manifest_count=0
while IFS= read -r manifest_line || [ -n "$manifest_line" ]; do
  hash=${manifest_line%%  *}
  name=${manifest_line#*  }
  [ "$hash" != "$manifest_line" ] || die "SHA256SUMS.txt 格式无效。"
  [ ${#hash} -eq 64 ] || die "SHA256SUMS.txt 包含无效 SHA-256。"
  printf '%s\n' "$hash" | grep -Eq '^[0-9a-f]{64}$' || die "SHA256SUMS.txt 包含无效 SHA-256。"
  printf '%s\n' "$EXPECTED_NAMES" | grep -Fx -- "$name" >/dev/null || die "SHA256SUMS.txt 包含未批准文件：$name"
  case "
$seen_names
" in
    *"
$name
"*) die "SHA256SUMS.txt 包含重复文件：$name" ;;
  esac
  [ -f "$SCRIPT_DIR/$name" ] && [ ! -L "$SCRIPT_DIR/$name" ] || die "软件包文件缺失或不是普通文件：$name"
  seen_names="${seen_names}${seen_names:+
}$name"
  manifest_count=$((manifest_count + 1))
done < "$MANIFEST"

[ "$manifest_count" -eq 9 ] || die "SHA256SUMS.txt 未覆盖完整软件包。"
for expected_name in $EXPECTED_NAMES; do
  case "
$seen_names
" in
    *"
$expected_name
"*) ;;
    *) die "SHA256SUMS.txt 缺少文件：$expected_name" ;;
  esac
done

package_entry_count=$(find "$SCRIPT_DIR" -mindepth 1 -maxdepth 1 -print | wc -l | tr -d '[:space:]')
regular_file_count=$(find "$SCRIPT_DIR" -mindepth 1 -maxdepth 1 -type f ! -type l -print | wc -l | tr -d '[:space:]')
[ "$package_entry_count" -eq 10 ] && [ "$regular_file_count" -eq 10 ] || \
  die "软件包根目录必须恰好包含 10 个已批准的普通文件。"

(cd "$SCRIPT_DIR" && shasum -a 256 -c SHA256SUMS.txt) || die "软件包 SHA-256 校验失败。"
file_output=$(file -b "$SCRIPT_DIR/grok-zh") || die "无法检查 grok-zh 文件类型。"
printf '%s\n' "$file_output" | grep -Eq "Mach-O .*executable ${BINARY_ARCH}" || die "grok-zh 不是 ${BINARY_ARCH} Mach-O 可执行文件。"
binary_archs=$(lipo -archs "$SCRIPT_DIR/grok-zh") || die "无法检查 grok-zh 架构。"
[ "$binary_archs" = "$BINARY_ARCH" ] || die "grok-zh 必须是纯 ${BINARY_ARCH} Mach-O，实际架构：$binary_archs"

version_output=$("$SCRIPT_DIR/grok-zh" --version 2>/dev/null) || die "软件包中的 grok-zh 无法运行。"
version_line=$(printf '%s\n' "$version_output" | sed -n '1p')
version=$(printf '%s\n' "$version_line" | sed -E -n 's/^grok-zh ([0-9A-Za-z.+-]+) \(.*/\1/p')
[ -n "$version" ] || die "无法从 grok-zh --version 读取版本。"
[ ${#version} -le 64 ] || die "软件包版本过长。"
semver_body='(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-((0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*)(\.(0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*))*))?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?'
semver_pattern="^${semver_body}$"
printf '%s\n' "$version" | grep -Eq "$semver_pattern" || die "软件包版本不是严格 SemVer：$version"

: "${HOME:?HOME 未设置}"
GROK_HOME_DIR=${GROK_HOME:-"$HOME/.grok"}
while [ "$GROK_HOME_DIR" != / ] && [ "${GROK_HOME_DIR%/}" != "$GROK_HOME_DIR" ]; do
  GROK_HOME_DIR=${GROK_HOME_DIR%/}
done
case "$GROK_HOME_DIR" in
  /*) ;;
  *) die "GROK_HOME 必须是绝对路径。" ;;
esac
case "$GROK_HOME_DIR/" in
  *'/../'*|*'/./'*|*'//'*|*:*|*'
'*) die "GROK_HOME 包含不安全的路径组件。" ;;
esac
[ "$GROK_HOME_DIR" != / ] || die "拒绝把根目录用作 GROK_HOME。"

GROK_HOME_PARENT=$(dirname "$GROK_HOME_DIR") || die "无法解析 GROK_HOME 父目录。"
[ -d "$GROK_HOME_PARENT" ] && [ ! -L "$GROK_HOME_PARENT" ] || \
  die "GROK_HOME 父目录必须预先存在且不能是符号链接：$GROK_HOME_PARENT"
physical_parent=$(CDPATH= cd -- "$GROK_HOME_PARENT" && pwd -P) || die "无法解析 GROK_HOME 父目录。"
[ "$physical_parent" = "$GROK_HOME_PARENT" ] || \
  die "GROK_HOME 父目录路径中包含符号链接：$GROK_HOME_PARENT"

ensure_secure_dir() {
  secure_dir=$1
  [ ! -L "$secure_dir" ] || die "拒绝使用符号链接目录：$secure_dir"
  if [ -e "$secure_dir" ]; then
    [ -d "$secure_dir" ] || die "安装路径不是目录：$secure_dir"
  else
    mkdir -m 700 "$secure_dir" || die "无法创建目录：$secure_dir"
  fi
  [ ! -L "$secure_dir" ] && [ -d "$secure_dir" ] || die "安装目录在创建时发生变化：$secure_dir"
  physical_dir=$(CDPATH= cd -- "$secure_dir" && pwd -P) || die "无法解析目录：$secure_dir"
  [ "$physical_dir" = "$secure_dir" ] || die "安装目录路径中包含符号链接：$secure_dir"
  [ "$(stat -f '%u' "$secure_dir")" = "$(id -u)" ] || die "安装目录不属于当前用户：$secure_dir"
  chmod 700 "$secure_dir" || die "无法保护目录权限：$secure_dir"
  [ "$(stat -f '%Lp' "$secure_dir")" = 700 ] || die "安装目录权限不是 0700：$secure_dir"
}

BIN_DIR="$GROK_HOME_DIR/bin"
DOWNLOAD_DIR="$GROK_HOME_DIR/grok-zh-downloads"
ensure_secure_dir "$GROK_HOME_DIR"
ensure_secure_dir "$BIN_DIR"
ensure_secure_dir "$DOWNLOAD_DIR"
acquire_install_lock

cleanup() {
  if [ -n "$STAGE_FILE" ]; then
    rm -f -- "$STAGE_FILE" 2>/dev/null || true
  fi
}
trap cleanup EXIT
trap 'cleanup; exit 1' HUP INT TERM

is_managed_canonical_target() {
  canonical_target=$1
  case "$canonical_target" in
    ../grok-zh-downloads/*) canonical_name=${canonical_target#../grok-zh-downloads/} ;;
    *) return 1 ;;
  esac
  case "$canonical_name" in
    ''|.|..|*/*|*\\*|*:*) return 1 ;;
  esac
  printf '%s\n' "$canonical_name" | grep -Eq "^grok-zh-${semver_body}-macos-${PACKAGE_ARCH}\\.[0-9A-Za-z-]+\\.installed$"
}

validate_managed_link() {
  managed_link=$1
  managed_kind=$2
  if [ -L "$managed_link" ]; then
    managed_target=$(readlink "$managed_link") || die "无法读取符号链接：$managed_link"
    case "$managed_kind:$managed_target" in
      agent-entry:grok-zh|grok-alias:grok-zh|agent-alias:agent-zh) return 0 ;;
    esac
    if { [ "$managed_kind" = canonical ] || [ "$managed_kind" = agent-entry ]; } && \
      is_managed_canonical_target "$managed_target"; then
      return 0
    fi
    die "拒绝覆盖不属于本安装器的符号链接：$managed_link -> $managed_target"
  elif [ -e "$managed_link" ]; then
    die "拒绝覆盖已有文件：$managed_link"
  fi
}

capture_link() {
  capture_path=$1
  if [ -L "$capture_path" ]; then
    readlink "$capture_path"
  else
    printf '%s\n' '__ABSENT__'
  fi
}

swap_link() {
  swap_target=$1
  swap_path=$2
  LINK_SEQ=$((LINK_SEQ + 1))
  swap_tmp="${swap_path}.$$.${LINK_SEQ}.tmp-link"
  [ ! -e "$swap_tmp" ] && [ ! -L "$swap_tmp" ] || return 1
  ln -s "$swap_target" "$swap_tmp" || return 1
  if ! mv -f "$swap_tmp" "$swap_path"; then
    rm -f -- "$swap_tmp" 2>/dev/null || true
    return 1
  fi
}

restore_link() {
  restore_path=$1
  restore_target=$2
  restore_expected=$3
  if [ ! -L "$restore_path" ] || [ "$(readlink "$restore_path")" != "$restore_expected" ]; then
    return 0
  fi
  if [ "$restore_target" = __ABSENT__ ]; then
    rm -f -- "$restore_path"
  else
    swap_link "$restore_target" "$restore_path"
  fi
}

GROK_ZH_LINK="$BIN_DIR/grok-zh"
AGENT_ZH_LINK="$BIN_DIR/agent-zh"
GROK_ALIAS_LINK="$BIN_DIR/grok"
AGENT_ALIAS_LINK="$BIN_DIR/agent"
validate_managed_link "$GROK_ZH_LINK" canonical
validate_managed_link "$AGENT_ZH_LINK" agent-entry
if [ "$WITH_COMPAT_ALIASES" -eq 1 ]; then
  validate_managed_link "$GROK_ALIAS_LINK" grok-alias
  validate_managed_link "$AGENT_ALIAS_LINK" agent-alias
fi

OLD_GROK_ZH=$(capture_link "$GROK_ZH_LINK")
OLD_AGENT_ZH=$(capture_link "$AGENT_ZH_LINK")
OLD_GROK_ALIAS=$(capture_link "$GROK_ALIAS_LINK")
OLD_AGENT_ALIAS=$(capture_link "$AGENT_ALIAS_LINK")

STAGE_FILE=$(mktemp "$DOWNLOAD_DIR/.grok-zh-stage.XXXXXX") || die "无法创建安装暂存文件。"
cp "$SCRIPT_DIR/grok-zh" "$STAGE_FILE" || die "无法复制 grok-zh。"
chmod 755 "$STAGE_FILE" || die "无法设置 grok-zh 执行权限。"
expected_binary_hash=$(awk '$2 == "grok-zh" { print $1 }' "$MANIFEST")
actual_binary_hash=$(shasum -a 256 "$STAGE_FILE" | awk '{ print $1 }')
[ "$actual_binary_hash" = "$expected_binary_hash" ] || die "暂存的 grok-zh 校验失败。"
stage_version_output=$("$STAGE_FILE" --version 2>/dev/null) || die "暂存的 grok-zh 无法运行。"
stage_version_line=$(printf '%s\n' "$stage_version_output" | sed -n '1p')
[ "$stage_version_line" = "$version_line" ] || die "暂存的 grok-zh 版本不一致。"

FINAL_RESERVATION=$(mktemp "$DOWNLOAD_DIR/grok-zh-$version-macos-${PACKAGE_ARCH}.XXXXXX") || die "无法预留版本目标。"
FINAL_FILE="${FINAL_RESERVATION}.installed"
if ! mv "$FINAL_RESERVATION" "$FINAL_FILE"; then
  rm -f -- "$FINAL_RESERVATION" 2>/dev/null || true
  die "无法预留最终版本目标。"
fi
if ! mv -f "$STAGE_FILE" "$FINAL_FILE"; then
  rm -f -- "$FINAL_FILE" 2>/dev/null || true
  die "无法发布已验证的 grok-zh。"
fi
STAGE_FILE=
FINAL_REL="../grok-zh-downloads/${FINAL_FILE##*/}"

applied_grok_zh=0
applied_agent_zh=0
applied_grok_alias=0
applied_agent_alias=0
rollback_links() {
  # If another installer/updater has already advanced the canonical pointer,
  # none of this process's earlier snapshots may be restored over that winner.
  if [ "$applied_grok_zh" -eq 1 ] && \
    { [ ! -L "$GROK_ZH_LINK" ] || [ "$(readlink "$GROK_ZH_LINK")" != "$FINAL_REL" ]; }; then
    return 0
  fi
  rollback_failed=0
  [ "$applied_agent_alias" -eq 0 ] || restore_link "$AGENT_ALIAS_LINK" "$OLD_AGENT_ALIAS" agent-zh || rollback_failed=1
  [ "$applied_grok_alias" -eq 0 ] || restore_link "$GROK_ALIAS_LINK" "$OLD_GROK_ALIAS" grok-zh || rollback_failed=1
  [ "$applied_agent_zh" -eq 0 ] || restore_link "$AGENT_ZH_LINK" "$OLD_AGENT_ZH" grok-zh || rollback_failed=1
  [ "$applied_grok_zh" -eq 0 ] || restore_link "$GROK_ZH_LINK" "$OLD_GROK_ZH" "$FINAL_REL" || rollback_failed=1
  [ "$rollback_failed" -eq 0 ]
}

abort_during_activation() {
  if ! rollback_links; then
    printf '%s\n' "${PROGRAM_NAME}: 激活被中断，入口链接未能完全回滚；请检查 $BIN_DIR。" >&2
  fi
  cleanup
  trap - EXIT HUP INT TERM
  exit 1
}
trap abort_during_activation HUP INT TERM

rollback_or_die() {
  rollback_message=$1
  if rollback_links; then
    die "$rollback_message；已回滚入口链接。已验证版本保留在 $FINAL_FILE。"
  fi
  die "$rollback_message；入口链接未能完全回滚，请检查 $BIN_DIR。已验证版本保留在 $FINAL_FILE。"
}

applied_grok_zh=1
if ! swap_link "$FINAL_REL" "$GROK_ZH_LINK"; then
  applied_grok_zh=0
  die "无法激活 grok-zh。已验证版本保留在 $FINAL_FILE。"
fi
applied_agent_zh=1
if ! swap_link grok-zh "$AGENT_ZH_LINK"; then
  rollback_or_die "无法激活 agent-zh"
fi
if [ "$WITH_COMPAT_ALIASES" -eq 1 ]; then
  applied_grok_alias=1
  if ! swap_link grok-zh "$GROK_ALIAS_LINK"; then
    rollback_or_die "无法创建 grok 兼容入口"
  fi
  applied_agent_alias=1
  if ! swap_link agent-zh "$AGENT_ALIAS_LINK"; then
    rollback_or_die "无法创建 agent 兼容入口"
  fi
fi

# All entry points now form one committed link graph. A later signal should
# only stop output, not undo a successfully activated installation.
trap 'cleanup; exit 1' HUP INT TERM

printf '\n%s\n' "✓ grok-zh v$version 安装成功。"
printf '%s\n' "安装目录：$BIN_DIR"
case ":${CALLER_PATH}:" in
  *":$BIN_DIR:"*) ;;
  *)
    printf '%s\n' '请把下面一行加入 shell 启动文件（例如 ~/.zshrc 或 ~/.bashrc）：'
    printf 'export PATH='; printf "'%s'" "$(printf '%s' "$BIN_DIR" | sed "s/'/'\\\\''/g")"; printf ':$PATH\n'
    ;;
esac
printf '%s\n' "重新打开终端后运行：grok-zh"
if [ "$WITH_COMPAT_ALIASES" -eq 1 ]; then
  printf '%s\n' "兼容入口已启用，也可以运行：grok"
fi
