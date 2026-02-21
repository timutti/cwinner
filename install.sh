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

# Get latest release tag via GitHub API
if command -v curl &>/dev/null; then
    LATEST=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' | sed 's/.*"tag_name": "\(.*\)".*/\1/')
elif command -v wget &>/dev/null; then
    LATEST=$(wget -qO- "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' | sed 's/.*"tag_name": "\(.*\)".*/\1/')
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

DL_DIR=$(mktemp -d)
trap 'rm -rf "${DL_DIR}"' EXIT

if command -v curl &>/dev/null; then
    HTTP_CODE=$(curl -sL -w '%{http_code}' "${URL}" -o "${DL_DIR}/cwinner.tar.gz")
    if [ "${HTTP_CODE}" != "200" ]; then
        echo "Error: download failed (HTTP ${HTTP_CODE}) for ${TARGET}"
        echo "  URL: ${URL}"
        exit 1
    fi
else
    wget -q "${URL}" -O "${DL_DIR}/cwinner.tar.gz"
fi

tar xzf "${DL_DIR}/cwinner.tar.gz" -C "${DL_DIR}"

mkdir -p "${INSTALL_DIR}"
mv "${DL_DIR}/cwinner" "${INSTALL_DIR}/cwinner"
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
