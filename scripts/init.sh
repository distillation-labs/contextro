#!/usr/bin/env bash
set -euo pipefail

echo "=== Contextro Init Check ==="

# Check Python version
python_version=$(python3 -c "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')")
echo "Python: $python_version"
if [[ "$(echo "$python_version < 3.10" | bc -l)" -eq 1 ]]; then
    echo "ERROR: Python >= 3.10 required"
    exit 1
fi

# Check package installs
echo ""
echo "--- Import Check ---"
python3 -c "import contextro_mcp; print('contextro_mcp OK')"
python3 -c "from contextro_mcp.core.models import Symbol, ParsedFile; print('core.models OK')"
python3 -c "from contextro_mcp.core.interfaces import IParser, IEngine; print('core.interfaces OK')"
python3 -c "from contextro_mcp.core.exceptions import ContextroException; print('core.exceptions OK')"
python3 -c "from contextro_mcp.core.graph_models import UniversalNode, NodeType; print('core.graph_models OK')"
python3 -c "from contextro_mcp.config import Settings; print('config OK')"

# Ruff
echo ""
echo "--- Ruff Check ---"
ruff check src/ tests/ || { echo "Ruff failed"; exit 1; }
echo "Ruff: clean"

# Pytest
echo ""
echo "--- Pytest ---"
pytest tests/ -v --tb=short || { echo "Tests failed"; exit 1; }

echo ""
echo "=== All checks passed ==="
