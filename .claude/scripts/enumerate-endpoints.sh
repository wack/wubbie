#!/usr/bin/env bash
#
# enumerate-endpoints.sh
#
# Enumerates all API endpoints defined in the application by generating
# and parsing the OpenAPI specification. Outputs each endpoint as a pair of
# (HTTP METHOD, Path).
#
# This script builds the project if necessary, generates the OpenAPI document
# using the application's export-openapi command, and extracts endpoint
# information using jq.
#
# Usage:
#   ./enumerate-endpoints.sh [OPTIONS]
#
# Options:
#   --release        Build and use release binary (default: debug)
#   --skip-build     Skip the cargo build step (assumes binary exists)
#   --json           Output as JSON array instead of plain text
#   --dry-run        Print commands without executing them
#   --help           Show this help message
#
# Output Format (default):
#   (GET, /api/v1/workspaces/{workspace_id}/api-keys)
#   (POST, /api/v1/workspaces/{workspace_id}/api-keys)
#   ...
#
# Output Format (--json):
#   [
#     {"method": "GET", "path": "/api/v1/workspaces/{workspace_id}/api-keys"},
#     ...
#   ]
#

set -euo pipefail

# =============================================================================
# CONSTANTS
# =============================================================================

# Color codes for output formatting. Using these consistently provides a
# unified visual experience and makes it easier to scan output for errors
# (red), warnings (yellow), successes (green), and informational messages (blue).
readonly COLOR_RED='\033[0;31m'
readonly COLOR_GREEN='\033[0;32m'
readonly COLOR_YELLOW='\033[0;33m'
readonly COLOR_BLUE='\033[0;34m'
readonly COLOR_RESET='\033[0m'

# Script location for relative path resolution
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR

# Project root is two levels up from .claude/scripts/
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
readonly PROJECT_ROOT

# =============================================================================
# GLOBAL STATE
# =============================================================================

# These variables are set by parse_arguments() and used throughout the script.
RELEASE_BUILD=false
SKIP_BUILD=false
JSON_OUTPUT=false
DRY_RUN=false

# =============================================================================
# LOGGING FUNCTIONS
#
# These functions provide consistent, color-coded output. Each function adds
# a tag prefix ([INFO], [OK], [WARN], [ERROR]) to make it easy to scan logs
# and identify message severity at a glance.
# =============================================================================

#######################################
# Prints an informational message in blue.
# Use for general progress updates and status information.
# Arguments:
#   $1 - The message to print
#######################################
info() {
    local -r message="$1"
    echo -e "${COLOR_BLUE}[INFO]${COLOR_RESET} ${message}" >&2
}

#######################################
# Prints a success message in green.
# Use when an operation completes successfully.
# Arguments:
#   $1 - The message to print
#######################################
success() {
    local -r message="$1"
    echo -e "${COLOR_GREEN}[OK]${COLOR_RESET} ${message}" >&2
}

#######################################
# Prints a warning message in yellow.
# Use for non-fatal issues or important notices that don't stop execution.
# Arguments:
#   $1 - The message to print
#######################################
warn() {
    local -r message="$1"
    echo -e "${COLOR_YELLOW}[WARN]${COLOR_RESET} ${message}" >&2
}

#######################################
# Prints an error message in red and exits with code 1.
# Use for fatal errors that prevent the script from continuing.
# The message is sent to stderr so it's visible even when stdout is redirected.
# Arguments:
#   $1 - The error message to print
#######################################
error_exit() {
    local -r message="$1"
    echo -e "${COLOR_RED}[ERROR]${COLOR_RESET} ${message}" >&2
    exit 1
}

# =============================================================================
# COMMAND EXECUTION
# =============================================================================

#######################################
# Executes a command, or prints it if in dry-run mode.
# This function checks the global DRY_RUN variable to determine behavior.
# In dry-run mode, the command is printed with a [DRY-RUN] prefix but not
# executed, and the function returns success (0).
#
# Arguments:
#   $@ - The command and its arguments to execute
# Returns:
#   The exit code of the command (0 in dry-run mode)
#######################################
run_cmd() {
    if [[ "${DRY_RUN}" == true ]]; then
        echo -e "${COLOR_YELLOW}[DRY-RUN]${COLOR_RESET} $*" >&2
        return 0
    else
        "$@"
    fi
}

# =============================================================================
# HELP AND USAGE
# =============================================================================

#######################################
# Prints the help message and exits.
#######################################
show_help() {
    cat << 'EOF'
Usage: enumerate-endpoints.sh [OPTIONS]

Enumerates all API endpoints defined in the application by generating
and parsing the OpenAPI specification. Outputs each endpoint as a pair of
(HTTP METHOD, Path).

Options:
  --release        Build and use release binary (default: debug)
  --skip-build     Skip the cargo build step (assumes binary exists)
  --json           Output as JSON array instead of plain text
  --dry-run        Print commands without executing them
  --help           Show this help message

Output Format (default):
  (GET, /api/v1/workspaces/{workspace_id}/api-keys)
  (POST, /api/v1/workspaces/{workspace_id}/api-keys)
  ...

Output Format (--json):
  [
    {"method": "GET", "path": "/api/v1/workspaces/{workspace_id}/api-keys"},
    ...
  ]

Examples:
  # Enumerate endpoints using debug build
  ./enumerate-endpoints.sh

  # Use release build for faster execution
  ./enumerate-endpoints.sh --release

  # Skip build if binary already exists
  ./enumerate-endpoints.sh --release --skip-build

  # Output as JSON for programmatic use
  ./enumerate-endpoints.sh --json

  # See what commands would be run
  ./enumerate-endpoints.sh --dry-run
EOF
    exit 0
}

# =============================================================================
# ARGUMENT PARSING
# =============================================================================

#######################################
# Parses command-line arguments and sets global configuration variables.
# Arguments:
#   $@ - All command-line arguments passed to the script
# Globals:
#   RELEASE_BUILD - Set to true if --release is provided
#   SKIP_BUILD    - Set to true if --skip-build is provided
#   JSON_OUTPUT   - Set to true if --json is provided
#   DRY_RUN       - Set to true if --dry-run is provided
#######################################
parse_arguments() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --release)
                RELEASE_BUILD=true
                shift
                ;;
            --skip-build)
                SKIP_BUILD=true
                shift
                ;;
            --json)
                JSON_OUTPUT=true
                shift
                ;;
            --dry-run)
                DRY_RUN=true
                shift
                ;;
            --help|-h)
                show_help
                ;;
            *)
                error_exit "Unknown option: $1. Use --help for usage information."
                ;;
        esac
    done
}

# =============================================================================
# PREREQUISITE CHECKS
#
# These checks verify that the local environment is properly configured.
# Failures here require manual intervention - the script cannot automatically
# recover from missing tools.
# =============================================================================

#######################################
# Verifies that cargo (Rust toolchain) is installed and available in PATH.
# cargo is required for building the application.
#######################################
check_cargo_available() {
    info "Checking if cargo is available..."

    if ! command -v cargo &>/dev/null; then
        error_exit "cargo is not installed. Please install the Rust toolchain and try again.
See: https://rustup.rs/"
    fi

    success "cargo is available"
}

#######################################
# Verifies that jq is installed and available in PATH.
# jq is required for parsing the OpenAPI JSON output.
#######################################
check_jq_available() {
    info "Checking if jq is available..."

    if ! command -v jq &>/dev/null; then
        error_exit "jq is not installed. Please install jq and try again.
See: https://jqlang.github.io/jq/download/"
    fi

    success "jq is available"
}

#######################################
# Runs all prerequisite checks.
#######################################
check_prerequisites() {
    info "=== Checking Prerequisites ==="

    # Only check cargo if we're going to build
    if [[ "${SKIP_BUILD}" == false ]]; then
        check_cargo_available
    fi

    check_jq_available
    success "All prerequisites satisfied"
    echo "" >&2
}

# =============================================================================
# BUILD FUNCTIONS
# =============================================================================

#######################################
# Builds the application.
# Uses release or debug mode based on the RELEASE_BUILD flag.
#######################################
build_application() {
    if [[ "${SKIP_BUILD}" == true ]]; then
        info "Skipping build step (--skip-build specified)"
        return 0
    fi

    info "=== Building Application ==="

    local build_args=("build" "-p" "__SERVICE_NAME__")

    if [[ "${RELEASE_BUILD}" == true ]]; then
        build_args+=("--release")
        info "Building in release mode..."
    else
        info "Building in debug mode..."
    fi

    cd "${PROJECT_ROOT}"
    run_cmd cargo "${build_args[@]}"

    success "Build completed"
    echo "" >&2
}

# =============================================================================
# OPENAPI EXPORT FUNCTIONS
# =============================================================================

#######################################
# Gets the path to the binary based on build mode.
# Outputs:
#   The path to the binary
#######################################
get_binary_path() {
    if [[ "${RELEASE_BUILD}" == true ]]; then
        echo "${PROJECT_ROOT}/target/release/__service_name__"
    else
        echo "${PROJECT_ROOT}/target/debug/__service_name__"
    fi
}

#######################################
# Verifies the binary exists at the expected location.
# Skips verification in dry-run mode since the binary may not exist.
#######################################
verify_binary_exists() {
    # Skip verification in dry-run mode
    if [[ "${DRY_RUN}" == true ]]; then
        return 0
    fi

    local binary_path
    binary_path=$(get_binary_path)

    if [[ ! -x "${binary_path}" ]]; then
        if [[ "${SKIP_BUILD}" == true ]]; then
            error_exit "Binary not found at ${binary_path}. Remove --skip-build to build first."
        else
            error_exit "Binary not found at ${binary_path} after build. Check for build errors."
        fi
    fi
}

#######################################
# Generates the OpenAPI specification and extracts endpoints.
# Outputs the endpoints to stdout in the requested format.
#######################################
export_and_parse_openapi() {
    info "=== Exporting OpenAPI Specification ==="

    local binary_path
    binary_path=$(get_binary_path)

    verify_binary_exists

    info "Generating OpenAPI document..."

    # In dry-run mode, show the command and output a sample
    if [[ "${DRY_RUN}" == true ]]; then
        run_cmd "${binary_path}" export-openapi
        echo "" >&2
        info "Would parse OpenAPI JSON and extract endpoints"
        if [[ "${JSON_OUTPUT}" == true ]]; then
            echo '[{"method": "GET", "path": "/example"}]'
        else
            echo "(GET, /example)"
        fi
        return 0
    fi

    # Generate OpenAPI spec and parse it
    local openapi_json
    if ! openapi_json=$("${binary_path}" export-openapi 2>/dev/null); then
        error_exit "Failed to generate OpenAPI specification. Ensure the application builds correctly."
    fi

    success "OpenAPI document generated"
    echo "" >&2

    info "=== Extracting Endpoints ==="

    # Parse the OpenAPI JSON to extract (method, path) pairs
    # The OpenAPI spec has a "paths" object where keys are paths and values
    # contain HTTP method objects (get, post, put, delete, etc.)
    local endpoints
    if ! endpoints=$(echo "${openapi_json}" | jq -r '
        .paths | to_entries[] | .key as $path |
        .value | to_entries[] |
        select(.key | test("^(get|post|put|patch|delete|head|options|trace)$")) |
        {method: .key | ascii_upcase, path: $path}
    '); then
        error_exit "Failed to parse OpenAPI specification. Ensure the JSON is valid."
    fi

    # Output in the requested format
    if [[ "${JSON_OUTPUT}" == true ]]; then
        echo "${endpoints}" | jq -s '.'
    else
        echo "${endpoints}" | jq -r '"(\(.method), \(.path))"'
    fi

    local count
    count=$(echo "${endpoints}" | jq -s 'length')
    success "Found ${count} endpoints" >&2
}

# =============================================================================
# MAIN
# =============================================================================

main() {
    parse_arguments "$@"
    check_prerequisites
    build_application
    export_and_parse_openapi
}

main "$@"
