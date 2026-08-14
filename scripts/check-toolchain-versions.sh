#!/usr/bin/env bash
#
# Verify that the Rust toolchain is pinned to the same version in both places.
#
#   rust-toolchain.toml  is read by rustup and cargo
#   .tool-versions       is read by asdf
#
# Nothing keeps these two in sync, and they are consumed by disjoint tools, so
# a bump to one silently leaves the other behind. Which version you actually
# get then depends on how you installed Rust.

set -euo pipefail

cd "$(dirname "$0")/.."

TOOLCHAIN_FILE="rust-toolchain.toml"
TOOL_VERSIONS_FILE=".tool-versions"

fail() {
    echo "error: $*" >&2
    exit 1
}

[ -f "$TOOLCHAIN_FILE" ] || fail "$TOOLCHAIN_FILE not found"
[ -f "$TOOL_VERSIONS_FILE" ] || fail "$TOOL_VERSIONS_FILE not found"

# channel = "1.93.1"  ->  1.93.1
toolchain_version=$(
    sed -n 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' \
        "$TOOLCHAIN_FILE" | head -1
)
[ -n "$toolchain_version" ] || fail "no channel found in $TOOLCHAIN_FILE"

# rust 1.93.1  ->  1.93.1
asdf_version=$(awk '$1 == "rust" { print $2; exit }' "$TOOL_VERSIONS_FILE")
[ -n "$asdf_version" ] || fail "no rust entry found in $TOOL_VERSIONS_FILE"

# asdf installs a concrete release; it cannot resolve a rustup channel alias.
case "$toolchain_version" in
stable | beta | nightly | nightly-*)
    fail "$TOOLCHAIN_FILE pins the '$toolchain_version' channel, but asdf needs a
       concrete version in $TOOL_VERSIONS_FILE. Pin an explicit release
       such as \"1.93.1\" in both files."
    ;;
esac

if [ "$toolchain_version" != "$asdf_version" ]; then
    cat >&2 <<EOF
error: the pinned Rust versions disagree

  $TOOLCHAIN_FILE   channel = "$toolchain_version"   (rustup, cargo)
  $TOOL_VERSIONS_FILE            rust $asdf_version          (asdf)

These files are read by different tools and nothing keeps them in sync.
Set both to the same version.
EOF
    exit 1
fi

echo "Rust toolchain pinned consistently at $toolchain_version"
