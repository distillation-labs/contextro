#!/bin/bash

# Script to commit changes one file at a time
# Usage: ./commit-one-by-one.sh [optional: "staged" or "unstaged"]
# If no argument provided, commits all changed files (both staged and unstaged)

set -e

MODE=${1:-"all"}

# Function to get commit message for a file
get_commit_message() {
    local file=$1
    # Default message based on file type
    local extension="${file##*.}"
    case "$extension" in
        py)
            echo "Update $file"
            ;;
        js|ts|tsx|jsx)
            echo "Update $file"
            ;;
        md)
            echo "Update documentation in $file"
            ;;
        sh)
            echo "Update script $file"
            ;;
        *)
            echo "Update $file"
            ;;
    esac
}

# Function to commit a single file
commit_file() {
    local file=$1
    local message=$(get_commit_message "$file")
    
    # Clear staging area first to ensure only this file gets committed
    git reset HEAD -- . 2>/dev/null
    echo "Adding $file..."
    git add "$file"
    
    echo "Committing with message: $message"
    git commit -m "$message"
    
    echo "✓ Committed $file"
    echo "---"
}

# Get list of changed files
if [ "$MODE" = "staged" ]; then
    echo "Committing staged files one by one..."
    files=$(git diff --cached --name-only --diff-filter=ACM)
elif [ "$MODE" = "unstaged" ]; then
    echo "Committing unstaged files one by one..."
    files=$(git diff --name-only --diff-filter=ACM)
else
    echo "Committing all changed files one by one..."
    # Get staged, unstaged, and untracked files
    staged=$(git diff --cached --name-only --diff-filter=ACM)
    unstaged=$(git diff --name-only --diff-filter=ACM)
    untracked=$(git ls-files --others --exclude-standard)
    files=$(echo -e "$staged\n$unstaged\n$untracked" | sort -u)
fi

# Check if there are any files to commit
if [ -z "$files" ]; then
    echo "No files to commit."
    exit 0
fi

# Count files
file_count=$(echo "$files" | grep -c . || echo 0)
echo "Found $file_count file(s) to commit."
echo "---"

# Commit each file
while IFS= read -r file; do
    if [ -n "$file" ]; then
        commit_file "$file"
    fi
done <<< "$files"

echo "All files committed successfully!"
