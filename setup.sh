#!/usr/bin/env bash
#
# Contextro Setup Script
# Sets up Python virtual environment and installs all dependencies
#
# Usage:
#   ./setup.sh              # Default: create venv + install with dev deps
#   ./setup.sh --clean      # Remove existing venv before creating new
#   ./setup.sh --prod       # Install production dependencies only (no dev)
#   ./setup.sh --reranker   # Include optional FlashRank reranker
#   ./setup.sh --no-verify  # Skip verification step
#   ./setup.sh --help       # Show this help message
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Configuration
VENV_DIR=".venv"
SUPPORTED_PYTHON_VERSIONS=("3.12" "3.11" "3.10")

# Parse arguments
CLEAN=false
PROD_ONLY=false
SKIP_VERIFY=false
RERANKER=false

print_usage() {
    echo -e "${CYAN}Contextro Setup Script${NC}"
    echo ""
    echo "Usage: ./setup.sh [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --clean      Remove existing virtual environment before creating new"
    echo "  --prod       Install production dependencies only (skip dev deps)"
    echo "  --reranker   Include optional FlashRank reranker"
    echo "  --no-verify  Skip verification step"
    echo "  --help       Show this help message"
    echo ""
    echo "Examples:"
    echo "  ./setup.sh                       # Dev install (pytest, ruff, mypy)"
    echo "  ./setup.sh --clean               # Remove old venv, create new one"
    echo "  ./setup.sh --prod                # Production-only install"
    echo "  ./setup.sh --reranker            # Dev install + FlashRank reranker"
    echo "  ./setup.sh --clean --prod        # Clean install, production only"
}

find_python() {
    if [[ -n "${CONTEXTRO_PYTHON:-}" ]]; then
        if [[ ! -x "${CONTEXTRO_PYTHON}" ]]; then
            echo -e "${RED}✗ CONTEXTRO_PYTHON is set but not executable: ${CONTEXTRO_PYTHON}${NC}"
            exit 1
        fi
        echo "${CONTEXTRO_PYTHON}"
        return 0
    fi

    local candidates=()
    for version in "${SUPPORTED_PYTHON_VERSIONS[@]}"; do
        candidates+=("python${version}")
    done
    candidates+=("python3" "python")

    for candidate in "${candidates[@]}"; do
        if ! command -v "${candidate}" >/dev/null 2>&1; then
            continue
        fi

        local version
        version=$("${candidate}" -c "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')")
        for supported in "${SUPPORTED_PYTHON_VERSIONS[@]}"; do
            if [[ "${version}" == "${supported}" ]]; then
                echo "${candidate}"
                return 0
            fi
        done
    done
}

for arg in "$@"; do
    case $arg in
        --clean)
            CLEAN=true
            shift
            ;;
        --prod)
            PROD_ONLY=true
            shift
            ;;
        --reranker)
            RERANKER=true
            shift
            ;;
        --no-verify)
            SKIP_VERIFY=true
            shift
            ;;
        --help|-h)
            print_usage
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $arg${NC}"
            print_usage
            exit 1
            ;;
    esac
done

echo -e "${CYAN}"
echo "╔═══════════════════════════════════════════════════════════╗"
echo "║             Contextro — Environment Setup                ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo -e "${NC}"

# Step 1: Check Python version
echo -e "${BLUE}[1/6]${NC} Checking Python version..."

PYTHON_CMD=$(find_python)

if [[ -z "${PYTHON_CMD}" ]]; then
    supported_versions=$(IFS=", "; echo "${SUPPORTED_PYTHON_VERSIONS[*]}")
    echo -e "${RED}✗ Python ${supported_versions} required. Install a supported version or set CONTEXTRO_PYTHON to its full path.${NC}"
    exit 1
fi

PYTHON_VERSION=$($PYTHON_CMD -c "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')")
PYTHON_MAJOR=$($PYTHON_CMD -c "import sys; print(sys.version_info.major)")
PYTHON_MINOR=$($PYTHON_CMD -c "import sys; print(sys.version_info.minor)")

if [[ $PYTHON_MAJOR -ne 3 ]] || [[ $PYTHON_MINOR -lt 10 ]] || [[ $PYTHON_MINOR -gt 12 ]]; then
    supported_versions=$(IFS=", "; echo "${SUPPORTED_PYTHON_VERSIONS[*]}")
    echo -e "${RED}✗ Python ${supported_versions} required. Found: ${PYTHON_VERSION}${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Python ${PYTHON_VERSION} detected (${PYTHON_CMD})${NC}"

# Step 2: Handle existing venv
echo -e "${BLUE}[2/6]${NC} Setting up virtual environment..."

if [[ -d "$VENV_DIR" ]]; then
    if [[ "$CLEAN" = true ]]; then
        echo -e "${YELLOW}  Removing existing virtual environment...${NC}"
        rm -rf "$VENV_DIR"
        echo -e "${GREEN}  ✓ Old venv removed${NC}"
    else
        echo -e "${YELLOW}  ⚠ Virtual environment already exists. Use --clean to recreate.${NC}"
    fi
fi

if [[ ! -d "$VENV_DIR" ]]; then
    echo -e "  Creating virtual environment in ${VENV_DIR}..."
    $PYTHON_CMD -m venv "$VENV_DIR"
    echo -e "${GREEN}  ✓ Virtual environment created${NC}"
else
    echo -e "${GREEN}  ✓ Using existing virtual environment${NC}"
fi

# Step 3: Activate venv and upgrade pip
echo -e "${BLUE}[3/6]${NC} Activating environment and upgrading pip..."

source "$VENV_DIR/bin/activate"

pip install --upgrade pip --quiet
echo -e "${GREEN}✓ pip upgraded to $(pip --version | awk '{print $2}')${NC}"

# Step 4: Check Rust toolchain and install dependencies
echo -e "${BLUE}[4/6]${NC} Checking Rust toolchain and installing dependencies..."

if ! command -v cargo >/dev/null 2>&1; then
    echo -e "${RED}✗ Rust toolchain not found (missing 'cargo').${NC}"
    echo -e "${YELLOW}  Source installs compile the bundled ctx_fast extension.${NC}"
    echo -e "${YELLOW}  Install Rust via https://rustup.rs/ and re-run setup.${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Rust toolchain detected ($(cargo --version | awk '{print $2}'))${NC}"

if [[ "$PROD_ONLY" = true ]]; then
    if [[ "$RERANKER" = true ]]; then
        echo -e "  Installing production + reranker dependencies..."
        pip install -e ".[reranker]" --quiet
    else
        echo -e "  Installing production dependencies..."
        pip install -e . --quiet
    fi
    echo -e "${GREEN}✓ Production dependencies installed${NC}"
else
    if [[ "$RERANKER" = true ]]; then
        echo -e "  Installing all dependencies (dev + reranker)..."
        pip install -e ".[dev,reranker]" --quiet
    else
        echo -e "  Installing all dependencies (including dev)..."
        pip install -e ".[dev]" --quiet
    fi
    echo -e "${GREEN}✓ All dependencies installed${NC}"
fi

# Step 5: Verify installation
if [[ "$SKIP_VERIFY" = true ]]; then
    echo -e "${BLUE}[5/6]${NC} ${YELLOW}Skipping verification (--no-verify)${NC}"
else
    echo -e "${BLUE}[5/6]${NC} Verifying installation..."

    if [[ -x "${VENV_DIR}/bin/contextro" ]]; then
        echo -e "${GREEN}  ✓ contextro CLI available${NC}"
    else
        echo -e "${RED}  ✗ contextro CLI not found${NC}"
        exit 1
    fi

    if python -c "import contextro_mcp; from contextro_mcp import ctx_fast; print('OK')" &> /dev/null; then
        echo -e "${GREEN}  ✓ Core imports verified (including ctx_fast)${NC}"
    else
        echo -e "${RED}  ✗ Import verification failed${NC}"
        exit 1
    fi

    if "${VENV_DIR}/bin/contextro" --version &> /dev/null; then
        echo -e "${GREEN}  ✓ contextro CLI responds${NC}"
    else
        echo -e "${RED}  ✗ contextro CLI verification failed${NC}"
        exit 1
    fi
fi

# Step 6: Print integration instructions
echo -e "${BLUE}[6/6]${NC} Setup complete!"

echo ""
echo -e "${GREEN}╔═══════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║                    Setup Complete! ✓                      ║${NC}"
echo -e "${GREEN}╚═══════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "To activate the environment:"
echo -e "  ${CYAN}source ${VENV_DIR}/bin/activate${NC}"
echo ""
echo -e "To start the server:"
echo -e "  ${CYAN}contextro${NC}"
echo ""
echo -e "To run a local HTTP smoke test:"
echo -e "  ${CYAN}CTX_TRANSPORT=http contextro --port 8000${NC}"
echo -e "  ${CYAN}python scripts/docker_healthcheck.py${NC}"
echo ""
echo -e "To add to Claude Code:"
echo -e "  ${CYAN}claude mcp add contextro -- ${PWD}/${VENV_DIR}/bin/contextro${NC}"
echo ""
if [[ "$PROD_ONLY" = false ]]; then
    echo -e "To run tests:"
    echo -e "  ${CYAN}pytest -v -m \"not slow\"${NC}"
    echo ""
fi
