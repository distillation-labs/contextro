# Contextro Publication Kit

This directory now contains the Rust-era manuscript and the 1,000-task `platform` study that backs it.

## Primary artifacts

- `contextro-paper.html` - current printable manuscript source
- `contextro-paper.pdf` - rendered PDF output
- `platform-1000-study-config.json` - benchmark configuration, tokenizer metadata, and index snapshot
- `platform-1000-study-tasks.json` - full deterministic 1,000-task inventory
- `platform-1000-study-results.json` - per-task, per-arm outcomes for all 2,000 executions
- `platform-1000-study-summary.json` - aggregate results cited by the paper
- `figures/contextro-system-overview.svg` - architecture figure used by the manuscript
- `figures/contextro-study-design.svg` - study-design figure used by the manuscript

## What changed in this refresh

- The paper now describes the current pure-Rust Contextro implementation.
- The benchmark is produced by the Rust `contextro-study` harness, not the older simulator.
- Core claims come from a deterministic 1,000-task paired study on the private `platform` repository.
- Graph-dependent tasks are explicitly excluded on this repo because the current TypeScript/JavaScript parser path produced zero relationships.

## Legacy supplemental artifacts

Older paired-study, public-proxy, schema-export, and social assets are still kept in this directory for historical reference. They are not the evidence base for the current paper.

## Reproduce the study

```bash
cd crates
cargo run --release -p contextro --bin contextro-study -- \
  --codebase /Users/japneetkalkat/platform \
  --output-dir /Users/japneetkalkat/conductor/workspaces/contextro/zurich/docs/publication \
  --tasks 1000
```

## Export notes

The HTML manuscript is designed to be rendered to PDF with a browser-based print pipeline.

Recommended approach:

1. Serve the repository root locally so relative assets resolve cleanly.
2. Open `http://127.0.0.1:8008/docs/publication/contextro-paper.html` in a Chromium-based browser.
3. Use print-to-PDF with background graphics enabled and browser headers disabled.
4. Save the output as `contextro-paper.pdf`.
