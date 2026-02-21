#!/usr/bin/env bash
set -euo pipefail

REPO="timutti/cwinner"
INSTALL_DIR="${HOME}/.local/bin"

# Detect OS
case "$(uname -s)" in
    Linux)  OS="unknown-linux-gnu" ;;
    Darwin) OS="apple-darwin" ;;
    *)
        echo "Error: unsupported OS: $(uname -s)"
        exit 1
        ;;
esac

# Detect architecture
case "$(uname -m)" in
    x86_64|amd64)   ARCH="x86_64" ;;
    aarch64|arm64)   ARCH="aarch64" ;;
    *)
        echo "Error: unsupported architecture: $(uname -m)"
        exit 1
        ;;
esac

TARGET="${ARCH}-${OS}"

# Get latest release tag
if command -v curl &>/dev/null; then
    LATEST=$(curl -sI "https://github.com/${REPO}/releases/latest" \
        | grep -i '^location:' | sed 's|.*/||' | tr -d '\r\n')
elif command -v wget &>/dev/null; then
    LATEST=$(wget -qS -O /dev/null "https://github.com/${REPO}/releases/latest" 2>&1 \
        | grep -i 'Location:' | tail -1 | sed 's|.*/||' | tr -d '\r\n')
else
    echo "Error: curl or wget required"
    exit 1
fi

if [ -z "${LATEST}" ]; then
    echo "Error: could not determine latest release"
    exit 1
fi

URL="https://github.com/${REPO}/releases/download/${LATEST}/cwinner-${TARGET}.tar.gz"

echo "Downloading cwinner ${LATEST} for ${TARGET}..."

TMPDIR=$(mktemp -d)
trap 'rm -rf "${TMPDIR}"' EXIT

if command -v curl &>/dev/null; then
    curl -sL "${URL}" -o "${TMPDIR}/cwinner.tar.gz"
else
    wget -q "${URL}" -O "${TMPDIR}/cwinner.tar.gz"
fi

tar xzf "${TMPDIR}/cwinner.tar.gz" -C "${TMPDIR}"

mkdir -p "${INSTALL_DIR}"
mv "${TMPDIR}/cwinner" "${INSTALL_DIR}/cwinner"
chmod +x "${INSTALL_DIR}/cwinner"

echo "Installed cwinner to ${INSTALL_DIR}/cwinner"

# Check if INSTALL_DIR is in PATH
if ! echo "${PATH}" | tr ':' '\n' | grep -qx "${INSTALL_DIR}"; then
    echo ""
    echo "Add to your PATH:"
    echo "  export PATH=\"${INSTALL_DIR}:\${PATH}\""
fi

echo ""
echo "Next: run 'cwinner install' to set up hooks and daemon"
