#!/bin/bash

# Script to bump version number in all relevant files
# Usage: ./bump-version.sh <new-version>
# Example: ./bump-version.sh 0.1.2

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored output
print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

# Check if version argument is provided
if [ -z "$1" ]; then
    print_error "Version number is required!"
    echo "Usage: $0 <new-version>"
    echo "Example: $0 0.1.2"
    exit 1
fi

NEW_VERSION="$1"

# Validate version format (semantic versioning: x.y.z)
if ! [[ "$NEW_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9]+)?$ ]]; then
    print_error "Invalid version format: $NEW_VERSION"
    echo "Version should follow semantic versioning: MAJOR.MINOR.PATCH (e.g., 0.1.2)"
    echo "Optional pre-release suffix is allowed (e.g., 0.1.2-alpha)"
    exit 1
fi

print_info "Bumping version to: $NEW_VERSION"
echo ""

# Get the project root directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Files to update
FILES=(
    "frontend/src-tauri/tauri.conf.json"
    "frontend/src-tauri/Cargo.toml"
    "frontend/package.json"
)

# Function to update version in JSON file
update_json_version() {
    local file="$1"
    local version="$2"
    
    if [ ! -f "$file" ]; then
        print_error "File not found: $file"
        return 1
    fi
    
    # Use sed to replace version in JSON (handles both "version": "x.y.z" and "version": "x.y.z",)
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS uses BSD sed
        sed -i '' "s/\"version\":[[:space:]]*\"[^\"]*\"/\"version\": \"$version\"/g" "$file"
    else
        # Linux uses GNU sed
        sed -i "s/\"version\":[[:space:]]*\"[^\"]*\"/\"version\": \"$version\"/g" "$file"
    fi
    
    print_info "Updated: $file"
}

# Function to update version in Cargo.toml
update_cargo_version() {
    local file="$1"
    local version="$2"
    
    if [ ! -f "$file" ]; then
        print_error "File not found: $file"
        return 1
    fi
    
    # Use sed to replace version = "x.y.z"
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS uses BSD sed
        sed -i '' "s/^version[[:space:]]*=[[:space:]]*\"[^\"]*\"/version = \"$version\"/g" "$file"
    else
        # Linux uses GNU sed
        sed -i "s/^version[[:space:]]*=[[:space:]]*\"[^\"]*\"/version = \"$version\"/g" "$file"
    fi
    
    print_info "Updated: $file"
}

# Update all files
print_info "Updating version in configuration files..."
echo ""

for file in "${FILES[@]}"; do
    if [[ "$file" == *.toml ]]; then
        update_cargo_version "$file" "$NEW_VERSION"
    else
        update_json_version "$file" "$NEW_VERSION"
    fi
done

echo ""
print_info "Version bump completed successfully!"
echo ""
print_info "Updated files:"
for file in "${FILES[@]}"; do
    echo "  - $file"
done
echo ""
print_warning "Note: Remember to commit these changes with:"
echo "  git add ${FILES[*]}"
echo "  git commit -m \"Bump version to $NEW_VERSION\""

