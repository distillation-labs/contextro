#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release-candidate.sh [options]

Build a local release candidate for Contextro, generate a wrapper with a fixed
CTX_STORAGE_DIR, scaffold external-repo testing artifacts, and optionally add
the wrapper to Claude Code as an MCP entry.

Options:
  --alias NAME           MCP alias to use for the generated wrapper (default: contextro-rc)
  --rc-dir DIR           Root directory for generated RC artifacts (default: scratch/release-candidate)
  --storage-dir DIR      Override CTX_STORAGE_DIR used by the generated wrapper
  --study-dir DIR        Override output directory for contextro-study results
  --tasks N              Number of tasks for each contextro-study run (default: 200)
  --repo PATH            External repository to include in the study pass (repeatable)
  --repos-file FILE      File containing external repo paths, one absolute path per line
  --install-claude       Attempt to run `claude mcp add` for the generated wrapper
  --skip-study           Skip the contextro-study pass even if repos are provided
  --help                 Show this help
EOF
}

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
CRATES_DIR="${ROOT_DIR}/crates"
DEFAULT_RC_DIR="${ROOT_DIR}/scratch/release-candidate"

ALIAS="contextro-rc"
RC_DIR="${DEFAULT_RC_DIR}"
STORAGE_DIR=""
STUDY_DIR=""
TASKS=200
INSTALL_CLAUDE=0
SKIP_STUDY=0
REPOS_FILE=""
declare -a REPOS=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --alias)
      ALIAS="${2:?missing value for --alias}"
      shift 2
      ;;
    --rc-dir)
      RC_DIR="${2:?missing value for --rc-dir}"
      shift 2
      ;;
    --storage-dir)
      STORAGE_DIR="${2:?missing value for --storage-dir}"
      shift 2
      ;;
    --study-dir)
      STUDY_DIR="${2:?missing value for --study-dir}"
      shift 2
      ;;
    --tasks)
      TASKS="${2:?missing value for --tasks}"
      shift 2
      ;;
    --repo)
      REPOS+=("${2:?missing value for --repo}")
      shift 2
      ;;
    --repos-file)
      REPOS_FILE="${2:?missing value for --repos-file}"
      shift 2
      ;;
    --install-claude)
      INSTALL_CLAUDE=1
      shift
      ;;
    --skip-study)
      SKIP_STUDY=1
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -z "${STORAGE_DIR}" ]]; then
  STORAGE_DIR="${RC_DIR}/storage"
fi
if [[ -z "${STUDY_DIR}" ]]; then
  STUDY_DIR="${RC_DIR}/study"
fi

mkdir -p "${RC_DIR}" "${STORAGE_DIR}" "${STUDY_DIR}" "${RC_DIR}/logs"

if [[ -n "${REPOS_FILE}" ]]; then
  if [[ ! -f "${REPOS_FILE}" ]]; then
    echo "Repos file not found: ${REPOS_FILE}" >&2
    exit 1
  fi
  while IFS= read -r line; do
    line="${line#"${line%%[![:space:]]*}"}"
    line="${line%"${line##*[![:space:]]}"}"
    [[ -z "${line}" ]] && continue
    [[ "${line}" =~ ^# ]] && continue
    REPOS+=("${line}")
  done < "${REPOS_FILE}"
fi

canonicalize() {
  local target="$1"
  if [[ -d "${target}" ]]; then
    (cd "${target}" && pwd)
  elif [[ -f "${target}" ]]; then
    local parent
    parent="$(cd "$(dirname "${target}")" && pwd)"
    printf '%s/%s\n' "${parent}" "$(basename "${target}")"
  else
    printf '%s\n' "${target}"
  fi
}

slugify_repo() {
  local repo_path="$1"
  local base
  base="$(basename "${repo_path}")"
  base="${base// /-}"
  base="${base//[^A-Za-z0-9._-]/-}"
  printf '%s\n' "${base}"
}

echo "==> Building Contextro release binaries"
(cd "${CRATES_DIR}" && cargo build --release -p contextro --bins)

CONTEXTRO_BIN="${CRATES_DIR}/target/release/contextro"
STUDY_BIN="${CRATES_DIR}/target/release/contextro-study"
BENCH_BIN="${CRATES_DIR}/target/release/contextro-bench"

if [[ ! -x "${CONTEXTRO_BIN}" ]]; then
  echo "Release binary not found: ${CONTEXTRO_BIN}" >&2
  exit 1
fi

WRAPPER_PATH="${RC_DIR}/${ALIAS}.sh"
INSTALL_HELPER="${RC_DIR}/install-${ALIAS}.sh"
CHECKLIST_PATH="${RC_DIR}/developer-gate-checklist.txt"
NEXT_STEPS_PATH="${RC_DIR}/NEXT_STEPS.txt"
REPO_TEMPLATE_PATH="${RC_DIR}/repos.txt"

cat > "${WRAPPER_PATH}" <<EOF
#!/usr/bin/env bash
set -euo pipefail
export CTX_STORAGE_DIR="${STORAGE_DIR}"
exec "${CONTEXTRO_BIN}" "\$@"
EOF
chmod +x "${WRAPPER_PATH}"

cat > "${INSTALL_HELPER}" <<EOF
#!/usr/bin/env bash
set -euo pipefail
cd "${ROOT_DIR}"
claude mcp remove "${ALIAS}" >/dev/null 2>&1 || true
claude mcp add "${ALIAS}" -- "${WRAPPER_PATH}"
EOF
chmod +x "${INSTALL_HELPER}"

if [[ ! -f "${REPO_TEMPLATE_PATH}" ]]; then
  cat > "${REPO_TEMPLATE_PATH}" <<'EOF'
# Add one absolute repository path per line.
# Example:
# /Users/you/src/project-one
# /Users/you/src/project-two
EOF
fi

cat > "${CHECKLIST_PATH}" <<'EOF'
Contextro release-candidate developer gate

For each external repo:
1. Index the repo with the local RC binary.
2. Verify wrong-path calls return explicit errors:
   - focus(path=...)
   - analyze(path=...)
   - code(operation="get_document_symbols", path=...)
3. Verify persistence-sensitive flows survive a restart:
   - compact(content=...) -> restart client/server -> retrieve(ref_id=...)
   - repo_add(path=...) -> restart client/server -> repo_status()
4. Verify search quality:
   - search() ordering looks sane
   - search_codebase_map(query/path) returns non-empty, relevant results
5. If the repo uses pytest, verify dead_code() does not flag fixtures.
6. Record any transcript where the MCP returns empty success-shaped output.
EOF

{
  echo "Contextro RC is ready."
  echo
  echo "Release binary:"
  echo "  ${CONTEXTRO_BIN}"
  echo
  echo "Wrapper for Claude Code / MCP clients:"
  echo "  ${WRAPPER_PATH}"
  echo
  echo "Persistent storage directory:"
  echo "  ${STORAGE_DIR}"
  echo
  echo "Claude add helper:"
  echo "  ${INSTALL_HELPER}"
  echo
  echo "Developer gate checklist:"
  echo "  ${CHECKLIST_PATH}"
  echo
  echo "Repo list template:"
  echo "  ${REPO_TEMPLATE_PATH}"
  echo
  echo "Suggested next commands:"
  echo "  ${INSTALL_HELPER}"
  echo "  ${ROOT_DIR}/scripts/release-candidate.sh --repos-file ${REPO_TEMPLATE_PATH}"
} > "${NEXT_STEPS_PATH}"

if [[ "${INSTALL_CLAUDE}" -eq 1 ]]; then
  if command -v claude >/dev/null 2>&1; then
    echo "==> Adding ${ALIAS} to Claude Code"
    if ! "${INSTALL_HELPER}"; then
      echo "WARNING: failed to add Claude MCP entry automatically." >&2
      echo "Run manually or choose a different alias with --alias." >&2
    fi
  else
    echo "WARNING: claude command not found; skipping Claude MCP installation." >&2
  fi
fi

if [[ "${SKIP_STUDY}" -eq 0 && "${#REPOS[@]}" -gt 0 ]]; then
  if [[ ! -x "${STUDY_BIN}" ]]; then
    echo "Study binary not found: ${STUDY_BIN}" >&2
    exit 1
  fi

  echo "==> Running contextro-study on external repos"
  for repo in "${REPOS[@]}"; do
    repo="$(canonicalize "${repo}")"
    if [[ ! -d "${repo}" ]]; then
      echo "WARNING: skipping missing repo ${repo}" >&2
      continue
    fi
    repo_slug="$(slugify_repo "${repo}")"
    repo_output="${STUDY_DIR}/${repo_slug}"
    mkdir -p "${repo_output}"
    echo "   -> ${repo}"
    "${STUDY_BIN}" --codebase "${repo}" --output-dir "${repo_output}" --tasks "${TASKS}" \
      | tee "${RC_DIR}/logs/${repo_slug}.study.log"
  done
fi

echo "==> Release-candidate assets created in ${RC_DIR}"
echo "==> Next steps are written to ${NEXT_STEPS_PATH}"
echo "==> Developer checklist is written to ${CHECKLIST_PATH}"
echo "==> Bench binary is available at ${BENCH_BIN}"
