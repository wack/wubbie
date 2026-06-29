#!/bin/bash
set -euo pipefail

# SessionStart hook for service template
# Installs CLI tools needed for development: ast-grep, cargo-make, cargo-nextest, shellcheck

# Only run in remote context (Claude Code for the Web)
# Skip installation on local machines where tools are typically already installed
if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ] && [ "${CLAUDE_CODE_ENTRYPOINT:-}" != "cloudweb" ]; then
  exit 0
fi

echo "Installing development tools..."

# Install cargo-binstall if not already installed (for fast binary downloads)
if ! command -v cargo-binstall &> /dev/null; then
  echo "Installing cargo-binstall..."
  curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
fi

# Install ast-grep if not already installed
if ! command -v ast-grep &> /dev/null; then
  echo "Installing ast-grep..."
  cargo binstall -y ast-grep || curl -fsSL https://raw.githubusercontent.com/ast-grep/ast-grep/main/install.sh | bash -s -- --to ~/.local/bin 2>/dev/null || echo "ast-grep installation skipped"
else
  echo "ast-grep already installed"
fi

# Install cargo-make if not already installed
if ! command -v cargo-make &> /dev/null; then
  echo "Installing cargo-make..."
  cargo binstall -y cargo-make
else
  echo "cargo-make already installed"
fi

# Install cargo-nextest if not already installed
if ! command -v cargo-nextest &> /dev/null; then
  echo "Installing cargo-nextest..."
  cargo binstall -y cargo-nextest
else
  echo "cargo-nextest already installed"
fi

# Install shellcheck if not already installed
if ! command -v shellcheck &> /dev/null; then
  echo "Installing shellcheck..."
  apt-get update && apt-get install -y shellcheck
else
  echo "shellcheck already installed"
fi

# Configure git authentication for private GitHub dependencies
# Uses GITHUB_TOKEN to rewrite GitHub URLs with token-based HTTPS auth,
# enabling cargo to fetch private git dependencies (e.g., the *-client crate).
if [ -n "${GITHUB_TOKEN:-}" ]; then
  git config --global url."https://x-access-token:${GITHUB_TOKEN}@github.com/".insteadOf "https://github.com/"
  git config --global --add url."https://x-access-token:${GITHUB_TOKEN}@github.com/".insteadOf "ssh://git@github.com/"
fi

# Ensure cargo bin directory is in PATH for this session
if [ -n "${CLAUDE_ENV_FILE:-}" ]; then
  echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> "$CLAUDE_ENV_FILE"
fi

echo "Development tools installation complete!"
