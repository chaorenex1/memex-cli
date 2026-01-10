#!/usr/bin/env bash
# memex-env: Cross-platform environment loader for .memex directory
# Version: 1.0.0
# Usage: memex-env [env_name|list]
#
# This script loads environment variables from $HOME/.memex/.env files
# and opens a new terminal session with those variables loaded.

set -e  # Exit on error

# ============================================================================
# Configuration
# ============================================================================

MEMEX_DIR="${HOME}/.memex"
DEFAULT_ENV_FILE=".env"
ENV_PREFIX=".env."

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# ============================================================================
# Helper Functions
# ============================================================================

print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1" >&2
}

show_help() {
    cat << EOF
memex-env - Environment Variable Loader

USAGE:
    memex-env [ENVIRONMENT] [OPTIONS]

OPTIONS:
    -s, --source        Load in current shell (recommended)
    -l, --list          List all available .env files
    -h, --help          Show this help message

COMMANDS:
    (none)          Load default .env file
    dev             Load .env.dev
    prod            Load .env.prod
    test            Load .env.test

EXAMPLES:
    memex-env                    # Load default .env in current shell
    memex-env dev                # Load .env.dev in current shell
    memex-env dev --source       # Load .env.dev (explicit)
    memex-env prod -s            # Load .env.prod (explicit)
    memex-env list               # List all .env files

BEHAVIOR:
    Loads environment variables into the CURRENT shell session.
    Variables persist until you close the terminal window.

ENVIRONMENT:
    MEMEX_DIR              Directory containing .env files (default: \$HOME/.memex)

FILES:
    \$HOME/.memex/.env      Default environment file
    \$HOME/.memex/.env.*    Environment-specific files

SECURITY:
    - Files are validated for path traversal attacks
    - File permissions are checked (warns if world-writable)
    - Protected system variables (PATH, HOME, etc.) are skipped
    - UTF-8 encoding is enforced

EOF
}

# ============================================================================
# Security Functions
# ============================================================================

validate_env_name() {
    local env_name="$1"

    # Check for path traversal attempts
    if [[ "$env_name" =~ \.\.|/ ]]; then
        print_error "Invalid environment name: '$env_name' (path traversal detected)"
        return 1
    fi

    # Check for invalid characters
    if [[ ! "$env_name" =~ ^[a-zA-Z0-9_-]+$ ]]; then
        print_error "Invalid environment name: '$env_name' (only alphanumeric, underscore, and hyphen allowed)"
        return 1
    fi

    return 0
}

check_file_permissions() {
    local file="$1"

    # Check if file is world-writable (security risk)
    if [[ -w "$file" ]]; then
        local perms=$(stat -c "%a" "$file" 2>/dev/null || stat -f "%Lp" "$file" 2>/dev/null)
        if [[ "$perms" =~ [2367]$ ]]; then
            print_warning "File '$file' is world-writable (permissions: $perms)"
            print_warning "Consider running: chmod 600 '$file'"
        fi
    fi
}

validate_env_file() {
    local file="$1"

    # Check if directory exists
    if [[ ! -d "$MEMEX_DIR" ]]; then
        print_error "Directory '$MEMEX_DIR' does not exist"
        print_info "Creating directory: $MEMEX_DIR"
        mkdir -p "$MEMEX_DIR"
        return 1
    fi

    # Check if file exists
    if [[ ! -f "$file" ]]; then
        print_error "Environment file '$file' not found"
        return 1
    fi

    # Check if file is readable
    if [[ ! -r "$file" ]]; then
        print_error "Cannot read file '$file' (permission denied)"
        return 1
    fi

    # Check file permissions
    check_file_permissions "$file"

    return 0
}

# ============================================================================
# Core Functions
# ============================================================================

list_env_files() {
    print_info "Available environment files in '$MEMEX_DIR':"
    echo ""

    local count=0

    # List default .env
    if [[ -f "$MEMEX_DIR/$DEFAULT_ENV_FILE" ]]; then
        echo "  • default (${DEFAULT_ENV_FILE})"
        ((count++))
    fi

    # List all .env.* files
    for file in "$MEMEX_DIR"/${ENV_PREFIX}*; do
        if [[ -f "$file" ]]; then
            local env_name=$(basename "$file" | sed "s/^${ENV_PREFIX}//")
            echo "  • $env_name ($ENV_PREFIX$env_name)"
            ((count++))
        fi
    done

    if [[ $count -eq 0 ]]; then
        print_warning "No .env files found in '$MEMEX_DIR'"
        print_info "Create a .env file with: echo 'VAR=value' > $MEMEX_DIR/.env"
    fi

    echo ""
}

get_env_file_path() {
    local env_name="$1"

    if [[ -z "$env_name" ]]; then
        echo "$MEMEX_DIR/$DEFAULT_ENV_FILE"
    else
        echo "$MEMEX_DIR/${ENV_PREFIX}${env_name}"
    fi
}

load_env_in_current_shell() {
    local env_file="$1"

    print_info "Loading environment in CURRENT shell..."
    echo ""

    # UTF-8 encoding
    export LANG=en_US.UTF-8
    export LC_ALL=en_US.UTF-8

    # Protected system variables
    local -a protected=("PATH" "HOME" "USER" "SHELL" "TERM" "PWD")
    local count=0

    # Load variables (simplified pattern)
    while IFS= read -r line || [[ -n "$line" ]]; do
        # Skip empty lines and comments
        [[ -z "$line" || "$line" =~ ^[[:space:]]*# ]] && continue

        # Simple validation: has '=' and non-empty key
        [[ ! "$line" =~ = ]] && continue

        # Split on first '='
        IFS='=' read -r key value <<< "$line"
        key=$(echo "$key" | xargs)
        value=$(echo "$value" | xargs)

        # Skip if empty key
        [[ -z "$key" ]] && continue

        # Check if variable is protected
        if [[ " ${protected[@]} " =~ " ${key} " ]]; then
            print_warning "Skipping protected: $key"
            continue
        fi

        # Export to current shell
        export "${key}=${value}" 2>/dev/null && {
            echo "  ${GREEN}✓${NC} $key"
            ((count++))
        }
    done < "$env_file"

    echo ""
    print_success "Loaded from: $env_file"
    print_success "$count variables exported to CURRENT shell"

    # Set tracking variables
    export MEMEX_ENV_LOADED="$(basename ${env_file})"
    export MEMEX_ENV_FILE="$env_file"
    export MEMEX_ENV_MODE="source"
}

# ============================================================================
# Main Function
# ============================================================================

main() {
    local env_name=""

    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case "$1" in
            help|--help|-h)
                show_help
                exit 0
                ;;
            list|--list|-l)
                list_env_files
                exit 0
                ;;
            -s|--source)
                # Explicit source mode (default behavior)
                shift
                ;;
            -*)
                print_error "Unknown option: $1"
                print_info "Run 'memex-env --help' for usage"
                exit 1
                ;;
            *)
                env_name="$1"
                shift
                ;;
        esac
    done

    # Validate environment name
    if [[ -n "$env_name" ]]; then
        if ! validate_env_name "$env_name"; then
            exit 1
        fi
    fi

    # Get and validate environment file
    local env_file=$(get_env_file_path "$env_name")
    if ! validate_env_file "$env_file"; then
        print_info "Run 'memex-env list' to see available environments"
        exit 1
    fi

    # Load environment in current shell
    load_env_in_current_shell "$env_file"
}

# ============================================================================
# Entry Point
# ============================================================================

main "$@"
