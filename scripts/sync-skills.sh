#!/usr/bin/env bash
# Sync SKILL.md files from .agent/skills/ to all platform directories.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SOURCE="$REPO_ROOT/.agent/skills"
PLATFORMS=(".github/skills" ".opencode/skills" ".kiro/skills")

for skill_dir in "$SOURCE"/*/; do
  [[ -f "$skill_dir/SKILL.md" ]] || continue
  name=$(basename "$skill_dir")
  for platform in "${PLATFORMS[@]}"; do
    dest="$REPO_ROOT/$platform/$name"
    mkdir -p "$dest"
    cp "$skill_dir/SKILL.md" "$dest/SKILL.md"
  done
  echo "synced: $name"
done

echo "done — synced to: ${PLATFORMS[*]}"
