#!/usr/bin/env bash
# Verify the same archive on its build host and on a native Intel consumer.
set -euo pipefail
archive="${1:?archive path required}"
: "${GROK_VERSION:?}" "${MACOS_ARCH:?}" "${MACOS_BINARY_ARCH:?}"
: "${PACKAGE_NAME:?}" "${RUNNER_TEMP:?}" "${GITHUB_RUN_ID:?}" "${GITHUB_RUN_ATTEMPT:?}"
[[ "$(uname -s)" == Darwin ]]
[[ "${PACKAGE_NAME}" == "grok-zh-${GROK_VERSION}-macos-${MACOS_ARCH}" ]]
[[ "${archive##*/}" == "${PACKAGE_NAME}.tar.gz" ]]
case "${MACOS_VERIFY_MODE:-native}" in
  native) [[ "$(uname -m)" == "${MACOS_BINARY_ARCH}" ]] ;;
  rosetta)
    [[ "$(uname -m)" == arm64 && "${MACOS_BINARY_ARCH}" == x86_64 ]]
    /usr/bin/arch -x86_64 /bin/sh -c 'test "$(/usr/bin/uname -m)" = x86_64'
    ;;
  *) echo '::error::Unknown macOS verification mode.'; exit 1 ;;
esac
run_installer() {
  if [[ "${MACOS_VERIFY_MODE:-native}" == rosetta ]]; then
    GROK_HOME="${install_home}" /usr/bin/arch -x86_64 /bin/sh "${package_root}/Install-GrokZh.sh" "$@"
  else
    GROK_HOME="${install_home}" "${package_root}/Install-GrokZh.sh" "$@"
  fi
}
(
  cd "$(dirname "${archive}")"
  shasum -a 256 -c "${archive##*/}.sha256"
)
verify_root="${RUNNER_TEMP}/grok-zh-macos-verify-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}"
if [[ -e "${verify_root}" ]]; then
  echo "::error::验证目录已经存在：${verify_root}"
  exit 1
fi
mkdir -p "${verify_root}"
COPYFILE_DISABLE=1 tar -xzf "${archive}" -C "${verify_root}"
package_root="${verify_root}/${PACKAGE_NAME}"
shopt -s nullglob dotglob
top_level_entries=("${verify_root}"/*)
shopt -u nullglob dotglob
only_top_level="${top_level_entries[0]:-}"
if (( ${#top_level_entries[@]} != 1 )) || \
   [[ "${only_top_level}" != "${package_root}" ]] || \
   [[ ! -d "${package_root}" || -L "${package_root}" ]]; then
  echo "::error::macOS 软件包必须只含同名顶层目录：${PACKAGE_NAME}"
  exit 1
fi
if [[ "$(stat -f '%Lp' "${package_root}")" != "755" ]]; then
  echo "::error::macOS 软件包顶层目录权限不是 0755。"
  exit 1
fi
expected_names="${verify_root}/expected-names.txt"
actual_names="${verify_root}/actual-names.txt"
printf '%s\n' \
  BUILD-INFO.txt \
  INSTALL-MACOS.md \
  Install-GrokZh.sh \
  LICENSE-grok-build.txt \
  NOTICE-third-party.txt \
  SHA256SUMS.txt \
  SOURCE_REV \
  THIRD-PARTY-NOTICES-xai-grok-tools.md \
  THIRD-PARTY-NOTICES.txt \
  grok-zh | LC_ALL=C sort > "${expected_names}"
shopt -s nullglob dotglob
package_entries=("${package_root}"/*)
shopt -u nullglob dotglob
printf '%s\n' "${package_entries[@]##*/}" | LC_ALL=C sort > "${actual_names}"
diff -u "${expected_names}" "${actual_names}"
for entry in "${package_entries[@]}"; do
  if [[ ! -f "${entry}" || -L "${entry}" ]]; then
    echo "::error::macOS 软件包根目录含有非普通文件：${entry}"
    exit 1
  fi
done
for name in grok-zh Install-GrokZh.sh; do
  if [[ "$(stat -f '%Lp' "${package_root}/${name}")" != "755" ]]; then
    echo "::error::${name} 权限不是 0755。"
    exit 1
  fi
done
for name in "${package_entries[@]##*/}"; do
  case "${name}" in grok-zh|Install-GrokZh.sh) continue ;; esac
  if [[ "$(stat -f '%Lp' "${package_root}/${name}")" != "644" ]]; then
    echo "::error::${name} 权限不是 0644。"
    exit 1
  fi
done
(
  cd "${package_root}"
  shasum -a 256 -c SHA256SUMS.txt
)
[[ "$(lipo -archs "${package_root}/grok-zh")" == "${MACOS_BINARY_ARCH}" ]]
codesign --verify --strict "${package_root}/grok-zh"
mkdir "${verify_root}/cli-home"
GROK_HOME="${verify_root}/cli-home" "${package_root}/grok-zh" --help >/dev/null
GROK_HOME="${verify_root}/cli-home" "${package_root}/grok-zh" agent --help >/dev/null
GROK_HOME="${verify_root}/cli-home" "${package_root}/grok-zh" update --help >/dev/null
verify_version_output="$("${package_root}/grok-zh" --version | head -n 1)"
if [[ "${verify_version_output}" != "grok-zh ${GROK_VERSION} ("* ]]; then
  echo "::error::解包后的 grok-zh 版本冒烟失败：${verify_version_output}"
  exit 1
fi

install_home="${RUNNER_TEMP}/grok-zh-install-smoke-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}"
if [[ -e "${install_home}" ]]; then
  echo "::error::安装器验证目录已经存在：${install_home}"
  exit 1
fi
run_installer
bin_dir="${install_home}/bin"
first_target="$(readlink "${bin_dir}/grok-zh")"
[[ "${first_target}" == ../grok-zh-downloads/grok-zh-*-macos-${MACOS_ARCH}.*.installed ]]
[[ "$(readlink "${bin_dir}/agent-zh")" == "grok-zh" ]]
[[ ! -e "${bin_dir}/grok" && ! -L "${bin_dir}/grok" ]]
[[ ! -e "${bin_dir}/agent" && ! -L "${bin_dir}/agent" ]]
installed_version="$("${bin_dir}/grok-zh" --version | head -n 1)"
[[ "${installed_version}" == "grok-zh ${GROK_VERSION} ("* ]]

run_installer --with-compat-aliases
second_target="$(readlink "${bin_dir}/grok-zh")"
[[ "${second_target}" == ../grok-zh-downloads/grok-zh-*-macos-${MACOS_ARCH}.*.installed ]]
[[ "${second_target}" != "${first_target}" ]]
[[ -f "${bin_dir}/${first_target}" ]]
[[ -f "${bin_dir}/${second_target}" ]]
alias_version="$("${bin_dir}/grok" --version | head -n 1)"
[[ "${alias_version}" == "grok-zh ${GROK_VERSION} ("* ]]
ls -lh "${archive}" "${archive}.sha256"
