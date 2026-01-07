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
    memex-env [ENVIRONMENT|COMMAND]

COMMANDS:
    (none)          Load default .env file
    dev             Load .env.dev
    prod            Load .env.prod
    test            Load .env.test
    list            List all available .env files
    help            Show this help message

EXAMPLES:
    memex-env               # Load default .env
    memex-env dev           # Load .env.dev
    memex-env prod          # Load .env.prod
    memex-env list          # List all .env files

ENVIRONMENT:
    MEMEX_DIR              Directory containing .env files (default: \$HOME/.memex)

FILES:
    \$HOME/.memex/.env      Default environment file
    \$HOME/.memex/.env.*    Environment-specific files

SECURITY:
    - Files are validated for path traversal attacks
    - File permissions are checked (warns if world-writable)
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

load_env_file() {
    local env_file="$1"
    local temp_script="$2"

    print_info "Loading environment from: $env_file"

    # Set UTF-8 encoding
    echo 'export LANG=en_US.UTF-8' >> "$temp_script"
    echo 'export LC_ALL=en_US.UTF-8' >> "$temp_script"

    # Parse .env file safely
    local line_num=0
    while IFS= read -r line || [[ -n "$line" ]]; do
        ((line_num++))

        # Skip empty lines and comments
        [[ -z "$line" || "$line" =~ ^[[:space:]]*# ]] && continue

        # Parse KEY=VALUE format
        if [[ "$line" =~ ^[[:space:]]*([A-Za-z_][A-Za-z0-9_]*)=(.*)$ ]]; then
            local key="${BASH_REMATCH[1]}"
            local value="${BASH_REMATCH[2]}"

            # Remove surrounding quotes if present
            value="${value#\"}"
            value="${value%\"}"
            value="${value#\'}"
            value="${value%\'}"

            # Export to temp script (safely quoted)
            echo "export ${key}=\"${value}\"" >> "$temp_script"
        else
            print_warning "Skipping invalid line $line_num: $line"
        fi
    done < "$env_file"

    print_success "Environment loaded successfully"
}

create_startup_script() {
    local env_file="$1"
    local temp_script=$(mktemp /tmp/memex-env.XXXXXX.sh)

    # Make script executable
    chmod 700 "$temp_script"

    # Add shebang
    echo '#!/usr/bin/env bash' > "$temp_script"
    echo '' >> "$temp_script"

    # Load environment variables
    load_env_file "$env_file" "$temp_script"

    # Add welcome message
    cat >> "$temp_script" << 'SCRIPT_EOF'

# Display loaded environment
echo ""
echo "╔════════════════════════════════════════════════════════════════╗"
echo "║  memex-env: Environment Variables Loaded                      ║"
echo "╚════════════════════════════════════════════════════════════════╝"
echo ""
echo "Loaded from: ENV_FILE_PLACEHOLDER"
echo "UTF-8 encoding: enabled"
echo ""
echo "Type 'exit' to return to the original shell"
echo ""

# Clean up this script on exit
trap 'rm -f "TEMP_SCRIPT_PLACEHOLDER"' EXIT

SCRIPT_EOF

    # Replace placeholders
    sed -i.bak "s|ENV_FILE_PLACEHOLDER|$env_file|g" "$temp_script"
    sed -i.bak "s|TEMP_SCRIPT_PLACEHOLDER|$temp_script|g" "$temp_script"
    rm -f "${temp_script}.bak"

    echo "$temp_script"
}

start_new_shell() {
    local startup_script="$1"

    # Detect current shell
    local current_shell=$(basename "$SHELL")

    print_info "Starting new $current_shell session..."

    # Start new shell with startup script
    if [[ "$current_shell" == "zsh" ]]; then
        exec zsh --rcs <(cat ~/.zshrc 2>/dev/null; cat "$startup_script")
    elif [[ "$current_shell" == "bash" ]]; then
        exec bash --rcfile <(cat ~/.bashrc 2>/dev/null; cat "$startup_script")
    else
        # Fallback: source the script in current shell
        print_warning "Unknown shell '$current_shell', sourcing in current session"
        source "$startup_script"
    fi
}

# ============================================================================
# Main Function
# ============================================================================

main() {
    local env_name=""

    # Parse arguments
    case "${1:-}" in
        help|--help|-h)
            show_help
            exit 0
            ;;
        list|--list|-l)
            list_env_files
            exit 0
            ;;
        "")
            # Use default .env
            env_name=""
            ;;
        *)
            # Use specified environment
            env_name="$1"
            ;;
    esac

    # Validate environment name
    if [[ -n "$env_name" ]]; then
        if ! validate_env_name "$env_name"; then
            exit 1
        fi
    fi

    # Get environment file path
    local env_file=$(get_env_file_path "$env_name")

    # Validate environment file
    if ! validate_env_file "$env_file"; then
        print_info "Run 'memex-env list' to see available environments"
        exit 1
    fi

    # Create startup script
    local startup_script=$(create_startup_script "$env_file")

    # Start new shell with environment loaded
    start_new_shell "$startup_script"
}

# ============================================================================
# Entry Point
# ============================================================================

main "$@"
