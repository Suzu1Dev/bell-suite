#!/bin/sh
# Standalone installer for BELL Suite (npm registry with automatic GitHub source fallback).

BELL_PACKAGE='bell-suite'
BELL_PRODUCT='BELL Suite'
BELL_BIN='bell-suite'
BELL_REPO='https://github.com/Suzu1Dev/bell-suite.git'
BELL_MIN_NODE=20

bell_message() {
  case "$bell_lang:$1" in
    zh-TW:help) printf '%s\n' "安裝 $BELL_PRODUCT（macOS、Linux、WSL）。需要時會自動安裝獨立的 Node.js 24。" \
      '用法：sh install.sh [--version <版本>] [--prefix <絕對路徑>] [--lang en|zh-TW]' \
      '  --version  指定版本（預設 latest；若 npm 尚未發布則自動從 GitHub 原始碼安裝）。' \
      '  --prefix   安裝目錄；預設為 $HOME/.local。' \
      '  --lang     訊息語言（en 或 zh-TW）。' \
      '  --help     顯示說明。' ;;
    en:help) printf '%s\n' "Install $BELL_PRODUCT on macOS, Linux or WSL. Bootstraps private Node.js 24 when needed." \
      'Usage: sh install.sh [--version <version>] [--prefix <absolute-path>] [--lang en|zh-TW]' \
      '  --version  Target version (defaults to latest; falls back to GitHub source if unpublished).' \
      '  --prefix   Installation prefix; defaults to $HOME/.local.' \
      '  --lang     Message language (en or zh-TW).' \
      '  --help     Show this help.' ;;
    zh-TW:argument) printf '無效或缺少的參數：%s。請用 --help 查看說明。\n' "$2" ;;
    en:argument) printf 'Invalid or missing argument: %s. Use --help for usage.\n' "$2" ;;
    zh-TW:prefix) printf '%s\n' '--prefix 必須是絕對路徑，且不能含冒號或換行。' ;;
    en:prefix) printf '%s\n' '--prefix must be an absolute path without colons or line breaks.' ;;
    zh-TW:os) printf '%s\n' '此安裝程式支援 macOS、Linux 與 WSL。' ;;
    en:os) printf '%s\n' 'This installer supports macOS, Linux and WSL.' ;;
    zh-TW:arch) printf '%s\n' '自動安裝 Node.js 支援 x64 與 arm64。請先手動安裝 Node.js 20+。' ;;
    en:arch) printf '%s\n' 'Automatic Node.js bootstrap supports x64 and arm64. Install Node.js 20+ first.' ;;
    zh-TW:tools) printf '%s\n' '需要 curl、tar、awk、mktemp 與 sha256sum（或 shasum）。' ;;
    en:tools) printf '%s\n' 'Requires curl, tar, awk, mktemp, and sha256sum (or shasum).' ;;
    zh-TW:node_installing) printf '正在下載並驗證官方 Node.js 24，安裝至 %s...\n' "$2" ;;
    en:node_installing) printf 'Downloading and verifying official Node.js 24 into %s...\n' "$2" ;;
    zh-TW:node_failed) printf '%s\n' '獨立 Node.js 安裝或驗證失敗。請檢查網路連線或自行安裝 Node.js 20+。' ;;
    en:node_failed) printf '%s\n' 'Private Node.js bootstrap failed. Check network or install Node.js 20+ manually.' ;;
    zh-TW:installing_npm) printf '正在從 npm 安裝 %s 至 %s...\n' "$2" "$3" ;;
    en:installing_npm) printf 'Installing %s from npm into %s...\n' "$2" "$3" ;;
    zh-TW:installing_git) printf 'npm 尚未發布此套件，正在從 GitHub (%s) 安裝至 %s...\n' "$BELL_REPO" "$2" ;;
    en:installing_git) printf 'Package not yet on npm; installing from GitHub (%s) into %s...\n' "$BELL_REPO" "$2" ;;
    zh-TW:git_missing) printf '%s\n' '從 GitHub 安裝需要 git，請先安裝 git 後重試。' ;;
    en:git_missing) printf '%s\n' 'git is required for GitHub source fallback. Install git and retry.' ;;
    zh-TW:failed) printf '%s\n' '安裝失敗，請檢查上方錯誤訊息與目錄權限（不需要 sudo）。' ;;
    en:failed) printf '%s\n' 'Installation failed. Check the output above and directory permissions (sudo is not needed).' ;;
    zh-TW:installed) printf '%s 安裝完成！可立即執行：\n' "$BELL_PRODUCT" ;;
    en:installed) printf '%s is installed! Run it now with:\n' "$BELL_PRODUCT" ;;
    zh-TW:check) printf '%s\n' '檢查環境與套件狀態：' ;;
    en:check) printf '%s\n' 'Check environment and workspace readiness:' ;;
    zh-TW:path) printf '%s\n' '若要在目前終端機直接使用指令名稱，請執行：' ;;
    en:path) printf '%s\n' 'To add the command to your current shell PATH, run:' ;;
  esac
}

bell_fail() { bell_message "$@" >&2; return 1; }

bell_quote() {
  printf "'"
  bell_q_rest=$1
  while :; do
    case "$bell_q_rest" in
      *"'"*)
        printf '%s' "${bell_q_rest%%"'"*}" "'\\''"
        bell_q_rest=${bell_q_rest#*"'"} ;;
      *) printf "%s'" "$bell_q_rest"; break ;;
    esac
  done
}

bell_runtime_ready() {
  command -v node >/dev/null 2>&1 && command -v npm >/dev/null 2>&1 || return 1
  bell_node=$(node -p 'process.platform + ":" + process.versions.node.split(".")[0]' </dev/null 2>/dev/null) || return 1
  case "$bell_node" in "$bell_platform":*) ;; *) return 1 ;; esac
  bell_major=${bell_node##*:}
  case "$bell_major" in ''|*[!0-9]*) return 1 ;; esac
  [ "$bell_major" -ge "$BELL_MIN_NODE" ] 2>/dev/null && npm --version </dev/null >/dev/null 2>&1
}

bell_bootstrap_node() (
  bell_arch=$(uname -m 2>/dev/null) || { bell_fail arch; exit 1; }
  case "$bell_arch" in x86_64|amd64) bell_arch=x64 ;; aarch64|arm64) bell_arch=arm64 ;; *) bell_fail arch; exit 1 ;; esac
  for bell_tool in curl tar awk mktemp mkdir rm; do
    command -v "$bell_tool" >/dev/null 2>&1 || { bell_fail tools; exit 1; }
  done
  if command -v sha256sum >/dev/null 2>&1; then bell_sum=sha256sum
  elif command -v shasum >/dev/null 2>&1; then bell_sum=shasum
  else bell_fail tools; exit 1; fi
  bell_tmp=$(mktemp -d "${TMPDIR:-/tmp}/bell-node.XXXXXXXX") || { bell_fail node_failed; exit 1; }
  bell_created=0
  trap 'rm -rf "$bell_tmp"; if [ "$bell_created" = 1 ]; then rm -rf "$bell_private_node"; fi' 0
  trap 'exit 1' 1 2 3 15
  bell_message node_installing "$bell_private_node"
  bell_base=https://nodejs.org/dist/latest-v24.x
  curl -fsSL --connect-timeout 15 --max-time 180 "$bell_base/SHASUMS256.txt" -o "$bell_tmp/SHASUMS256.txt" </dev/null || { bell_fail node_failed; exit 1; }
  bell_entry=$(awk -v platform="$bell_platform" -v arch="$bell_arch" '
    $2 ~ ("^node-v24\\.[0-9]+\\.[0-9]+-" platform "-" arch "\\.tar\\.gz$") && length($1) == 64 { print $1, $2; count++ }
    END { if (count != 1) exit 1 }
  ' "$bell_tmp/SHASUMS256.txt") || { bell_fail node_failed; exit 1; }
  bell_expected=${bell_entry%% *}
  bell_archive=${bell_entry#* }
  curl -fsSL --connect-timeout 15 --max-time 180 "$bell_base/$bell_archive" -o "$bell_tmp/node.tar.gz" </dev/null || { bell_fail node_failed; exit 1; }
  if [ "$bell_sum" = sha256sum ]; then bell_actual=$(sha256sum "$bell_tmp/node.tar.gz") || exit 1
  else bell_actual=$(shasum -a 256 "$bell_tmp/node.tar.gz") || exit 1; fi
  [ "${bell_actual%% *}" = "$bell_expected" ] || { bell_fail node_failed; exit 1; }
  mkdir -p "${bell_private_node%/*}" && mkdir "$bell_private_node" || { bell_fail node_failed; exit 1; }
  bell_created=1
  tar -xzf "$bell_tmp/node.tar.gz" --strip-components=1 -C "$bell_private_node" </dev/null || { bell_fail node_failed; exit 1; }
  [ -x "$bell_private_node/bin/node" ] && [ -x "$bell_private_node/bin/npm" ] && bell_runtime_ready || { bell_fail node_failed; exit 1; }
  bell_created=0
)

bell_install_from_git() {
  command -v git >/dev/null 2>&1 || { bell_fail git_missing; return 1; }
  bell_app_dir="${bell_prefix%/}/share/$BELL_BIN/app"
  bell_bin_dir="${bell_prefix%/}/bin"
  bell_message installing_git "$bell_app_dir"
  mkdir -p "${bell_app_dir%/*}" "$bell_bin_dir" || { bell_fail failed; return 1; }
  if [ -d "$bell_app_dir/.git" ]; then
    git -C "$bell_app_dir" fetch --depth 1 origin main </dev/null &&
      git -C "$bell_app_dir" reset --hard FETCH_HEAD </dev/null || { bell_fail failed; return 1; }
  else
    rm -rf "$bell_app_dir"
    git clone --depth 1 "$BELL_REPO" "$bell_app_dir" </dev/null || { bell_fail failed; return 1; }
  fi
  (cd "$bell_app_dir" && npm install --no-audit --no-fund </dev/null) || { bell_fail failed; return 1; }
  if node -e 'const p=require("./package.json"); process.exit(p.scripts && p.scripts.build ? 0 : 1)' --prefix "$bell_app_dir" >/dev/null 2>&1; then
    (cd "$bell_app_dir" && npm run build </dev/null) || { bell_fail failed; return 1; }
  elif (cd "$bell_app_dir" && node -e 'const fs=require("fs"); const p=JSON.parse(fs.readFileSync("package.json","utf8")); process.exit(p.scripts && p.scripts.build ? 0 : 1)') </dev/null 2>/dev/null; then
    (cd "$bell_app_dir" && npm run build </dev/null) || { bell_fail failed; return 1; }
  fi
  chmod +x "$bell_app_dir/bin/$BELL_BIN.mjs" || { bell_fail failed; return 1; }
  if [ -n "$bell_node_bin" ]; then
    cat > "$bell_bin_dir/$BELL_BIN" <<EOF
#!/bin/sh
export PATH="$bell_node_bin:\$PATH"
exec "$bell_app_dir/bin/$BELL_BIN.mjs" "\$@"
EOF
    chmod +x "$bell_bin_dir/$BELL_BIN" || { bell_fail failed; return 1; }
  else
    ln -sf "$bell_app_dir/bin/$BELL_BIN.mjs" "$bell_bin_dir/$BELL_BIN" || { bell_fail failed; return 1; }
  fi
}

bell_install_main() {
  case "${LANG-}" in zh_TW*|zh-TW*) bell_lang=zh-TW ;; *) bell_lang=en ;; esac
  bell_prefix=${HOME:+$HOME/.local}
  bell_version=latest
  bell_help=0
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --lang)
        [ "$#" -ge 2 ] || { bell_fail argument "$1"; return 1; }
        case "$2" in en|zh-TW) bell_lang=$2 ;; *) bell_fail argument "$1"; return 1 ;; esac
        shift 2 ;;
      --prefix)
        [ "$#" -ge 2 ] || { bell_fail argument "$1"; return 1; }
        bell_prefix=$2; shift 2 ;;
      --version)
        [ "$#" -ge 2 ] || { bell_fail argument "$1"; return 1; }
        bell_version=$2; shift 2 ;;
      --help|-h) bell_help=1; shift ;;
      *) bell_fail argument "$1"; return 1 ;;
    esac
  done
  if [ "$bell_help" = 1 ]; then bell_message help; return 0; fi
  case "$bell_prefix" in /*) ;; *) bell_fail prefix; return 1 ;; esac
  case "$bell_prefix" in *:*|*'
'*) bell_fail prefix; return 1 ;; esac
  bell_os=$(uname -s 2>/dev/null) || { bell_fail os; return 1; }
  case "$bell_os" in Darwin) bell_platform=darwin ;; Linux) bell_platform=linux ;; *) bell_fail os; return 1 ;; esac
  bell_original_path=${PATH-}
  bell_node_bin=
  if ! bell_runtime_ready; then
    bell_private_node=${bell_prefix%/}/share/$BELL_PACKAGE/node
    bell_node_bin=$bell_private_node/bin
    PATH=$bell_node_bin:$bell_original_path
    export PATH
    if [ ! -x "$bell_node_bin/node" ] || ! bell_runtime_ready; then
      rm -rf "$bell_private_node"
      bell_bootstrap_node || return 1
    fi
  fi

  bell_spec="$BELL_PACKAGE@$bell_version"
  bell_bin_dir=${bell_prefix%/}/bin
  bell_command=$bell_bin_dir/$BELL_BIN
  if npm view "$bell_spec" version </dev/null >/dev/null 2>&1; then
    bell_message installing_npm "$bell_spec" "$bell_prefix"
    npm install --global --prefix "$bell_prefix" --no-audit --no-fund -- "$bell_spec" </dev/null || bell_install_from_git || return 1
  else
    bell_install_from_git || return 1
  fi

  [ -x "$bell_command" ] || { bell_fail failed; return 1; }
  bell_message installed
  case ":$bell_original_path:" in
    *":$bell_bin_dir:"*) ;;
    *) bell_message path; printf 'export PATH='; bell_quote "$bell_bin_dir"; printf ':"$PATH"\n' ;;
  esac
  bell_quote "$bell_command"; printf '\n'
  bell_message check
  bell_quote "$bell_command"; printf ' doctor\n'
}

bell_install_main "$@"
