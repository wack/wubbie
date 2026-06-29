#!/usr/bin/env bash
set -euo pipefail

# Service Template Initialization Script
# This script replaces placeholder values with your service name.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_header() {
    echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║${NC}          Service Template Initialization Script            ${BLUE}║${NC}"
    echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
    echo ""
}

print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}!${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

print_info() {
    echo -e "${BLUE}→${NC} $1"
}

# Validate service name: lowercase alphanumeric and hyphens only, must start with letter
validate_service_name() {
    local name="$1"

    if [[ -z "$name" ]]; then
        print_error "Service name cannot be empty"
        return 1
    fi

    if [[ ! "$name" =~ ^[a-z][a-z0-9-]*$ ]]; then
        print_error "Service name must start with a lowercase letter and contain only lowercase letters, numbers, and hyphens"
        return 1
    fi

    if [[ "$name" =~ --|-$ ]]; then
        print_error "Service name cannot have consecutive hyphens or end with a hyphen"
        return 1
    fi

    if [[ ${#name} -gt 63 ]]; then
        print_error "Service name must be 63 characters or less (Kubernetes DNS naming limit)"
        return 1
    fi

    return 0
}

# Convert kebab-case to snake_case for Rust identifiers
to_snake_case() {
    echo "$1" | tr '-' '_'
}

# Replace placeholders in files
replace_in_file() {
    local file="$1"
    local search="$2"
    local replace="$3"

    if [[ -f "$file" ]]; then
        if grep -q "$search" "$file" 2>/dev/null; then
            if [[ "$OSTYPE" == "darwin"* ]]; then
                sed -i '' "s|$search|$replace|g" "$file"
            else
                sed -i "s|$search|$replace|g" "$file"
            fi
            return 0
        fi
    fi
    return 1
}

# Main replacement function
perform_replacements() {
    local service_name="$1"
    local snake_case_name="$2"
    local files_modified=0

    print_info "Replacing __SERVICE_NAME__ with ${service_name}"
    print_info "Replacing __service_name__ with ${snake_case_name}"
    echo ""

    # Find all files that need replacement (excluding .git, target, node_modules)
    while IFS= read -r -d '' file; do
        local modified=false

        if replace_in_file "$file" "__SERVICE_NAME__" "$service_name"; then
            modified=true
        fi

        if replace_in_file "$file" "__service_name__" "$snake_case_name"; then
            modified=true
        fi

        if [[ "$modified" == true ]]; then
            local relative_path="${file#$PROJECT_ROOT/}"
            print_success "Updated: $relative_path"
            ((files_modified++)) || true
        fi
    done < <(find "$PROJECT_ROOT" \
        -type f \
        ! -path "*/.git/*" \
        ! -path "*/target/*" \
        ! -path "*/node_modules/*" \
        ! -name "init-service.sh" \
        -print0)

    echo ""
    print_success "Modified $files_modified files"
}

# Rename helm chart directory name in Chart.yaml
rename_chart() {
    local service_name="$1"
    local chart_yaml="$PROJECT_ROOT/helm/chart/Chart.yaml"

    if [[ -f "$chart_yaml" ]]; then
        print_info "Helm chart configured with name: ${service_name}"
    fi
}

# Rename crates/client -> crates/<service>-client so the directory matches the
# client package name (__SERVICE_NAME__-client), and fix the workspace members
# path. Matches the keystore-client / metricstore-client convention.
rename_client_crate() {
    local service_name="$1"
    local old_dir="$PROJECT_ROOT/crates/client"
    local new_dir="$PROJECT_ROOT/crates/${service_name}-client"

    if [[ ! -d "$old_dir" ]]; then
        print_info "No crates/client directory to rename"
        return 0
    fi
    if [[ -e "$new_dir" ]]; then
        print_warning "crates/${service_name}-client already exists; skipping client rename"
        return 0
    fi

    mv "$old_dir" "$new_dir"
    replace_in_file "$PROJECT_ROOT/Cargo.toml" "crates/client" "crates/${service_name}-client"
    print_success "Renamed client crate -> crates/${service_name}-client"
}

# Self-destruct option
cleanup_template_files() {
    local response
    echo ""
    read -p "Remove template initialization files? (y/N) " response

    if [[ "$response" =~ ^[Yy]$ ]]; then
        rm -f "$PROJECT_ROOT/TEMPLATE.md"
        rm -f "$PROJECT_ROOT/scripts/init-service.sh"
        rmdir "$PROJECT_ROOT/scripts" 2>/dev/null || true
        print_success "Template files removed"
    else
        print_info "Template files kept (you can delete them manually later)"
    fi
}

# Display next steps
print_next_steps() {
    local service_name="$1"

    echo ""
    echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║${NC}                        Next Steps                          ${BLUE}║${NC}"
    echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo "  1. Review the changes made to the repository"
    echo ""
    echo "  2. Update CLAUDE.md with your service-specific guidelines"
    echo ""
    echo "  3. Start the development database:"
    echo "     cargo make pg"
    echo ""
    echo "  4. Run migrations:"
    echo "     cargo make migrate"
    echo ""
    echo "  5. Build and run the server:"
    echo "     cargo run -- server"
    echo ""
    echo "  6. Replace example business logic in:"
    echo "     - src/domain/       (domain entities)"
    echo "     - src/services/     (business logic)"
    echo "     - src/repos/        (data access)"
    echo "     - src/controllers/  (HTTP handlers)"
    echo "     - src/views/        (request/response types)"
    echo "     - crates/migrations/ (database schema)"
    echo ""
}

# Main entry point
main() {
    print_header

    local service_name="${1:-}"

    # Prompt for service name if not provided
    if [[ -z "$service_name" ]]; then
        echo "Enter your service name (lowercase, alphanumeric, hyphens allowed):"
        echo "Example: user-service, auth-api, payment-gateway"
        echo ""
        read -p "Service name: " service_name
    fi

    echo ""

    # Validate
    if ! validate_service_name "$service_name"; then
        exit 1
    fi

    local snake_case_name
    snake_case_name=$(to_snake_case "$service_name")

    print_info "Service name: ${service_name}"
    print_info "Rust crate name: ${snake_case_name}"
    echo ""

    # Confirm before proceeding
    read -p "Proceed with initialization? (Y/n) " confirm
    if [[ "$confirm" =~ ^[Nn]$ ]]; then
        print_warning "Initialization cancelled"
        exit 0
    fi

    echo ""

    # Perform replacements
    perform_replacements "$service_name" "$snake_case_name"

    # Update configurations
    rename_chart "$service_name"
    rename_client_crate "$service_name"

    # Optional cleanup
    cleanup_template_files

    # Print next steps
    print_next_steps "$service_name"

    print_success "Service template initialized successfully!"
}

main "$@"
