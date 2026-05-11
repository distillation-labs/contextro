# Contextro Publication Kit

This directory contains the manuscript source, publication-ready evidence artifacts, social copy, and image assets for publishing Contextro as a research paper plus launch content.

## Contents

- `contextro-paper.html` - full paper source in printable HTML
- `contextro-paper.pdf` - rendered manuscript PDF
- `publication-manifest.json` - map of paper files and social assets
- `paired-study-tasks.json` - sanitized machine-readable full 60-task paired-study inventory
- `paired-study-comparable-tasks.json` - sanitized machine-readable 39-task comparable subset used for strict control-arm analysis
- `paired-study-rerun-summary.json` - fresh rerun of the 60-task paired study
- `paired-study-rerun-config.json` - configuration used for the fresh paired-study rerun
- `paired-study-subset-robustness.json` - deterministic 100-subset robustness analysis over the 39 comparable tasks
- `open-token-benchmark.json` - fresh public token-efficiency rerun on `src/`
- `open-retrieval-quality.json` - fresh public retrieval-quality rerun on `src/`
- `public-proxy-comparable-tasks.json` - machine-readable 39-task public proxy discovery suite
- `public-proxy-edit-tasks.json` - machine-readable 8-task deterministic discovery-plus-edit suite
- `public-proxy-strong-baseline.json` - stronger local-tools versus Contextro rerun on the public proxy repository
- `edit-correctness-benchmark.json` - deterministic edit correctness benchmark results
- `contextro-tool-api-schemas.json` - exact request/response schemas and example payloads for 30 tools
- `figures/` - publication-specific figures used by the manuscript
- `social/` - platform copy and generated image assets

## Figure Assets

The paper uses a mix of publication-specific figures and existing project figures.

Publication-specific figures in `figures/`:

- `contextro-system-overview.svg`
- `contextro-study-design.svg`

Existing project figures reused from `docs/publication/figures/`:

- `01_headline_metrics.svg`
- `02_token_by_category.svg`
- `06_autoresearch_loop.png`

## Manuscript Structure

The manuscript now follows a more formal academic structure inspired by MIT, Harvard, and Yale guidance:

- title page
- publication note and license page
- abstract page
- table of contents
- list of figures
- list of tables
- main body with numbered sections and subsections
- references
- appendices
- index

## Reproducibility Artifacts

The revised manuscript now distinguishes between five evidence layers:

- archived aggregate launch-era summaries in `scripts/experiment_results/summary.json`
- fresh paired-study aggregate rerun artifacts copied into this directory
- fresh public benchmark reruns on the public `src/` tree
- public proxy benchmark artifacts and task catalogs bundled in this directory
- supplemental methodology artifacts for the stronger-baseline proxy rerun, deterministic edit correctness benchmark, and tool-schema export

The paired-study task catalogs are exported directly by `scripts/experiment_platform.py` and are designed to satisfy the paper checklist requirement for a machine-readable task inventory without disclosing private repository identifiers. `paired-study-tasks.json` preserves the full 60-task suite in sanitized form, while `paired-study-comparable-tasks.json` isolates the 39 tasks with scripted grep-plus-read control equivalents.

All refreshed token metrics in this package now use `tiktoken` with the `cl100k_base` encoding. The paired-study rerun config and public benchmark JSON files record that tokenizer metadata explicitly.

The robustness file uses 100 deterministic bootstrap task subsets over the 39 directly comparable tasks. The seed and sampling method are recorded inside `paired-study-subset-robustness.json`.

The deterministic edit benchmark is intentionally narrow: it validates correct file discovery, exact rewrite application, and post-edit Python syntax validity on eight scripted tasks. It is included to close the paper's end-to-end correctness gap, not to overclaim a full autonomous-agent benchmark.

The schema export publishes exact machine-readable contracts for 30 tools. Appendix B in the paper stays human-readable, while `contextro-tool-api-schemas.json` carries JSON Schema-compatible inputs, outputs, annotations, and example payloads.

## Social Assets

Generated image previews currently live beside the SVG sources in `social/`:

- `contextro-social-hero.svg`
- `contextro-social-hero.png`
- `contextro-linkedin-card.svg`
- `contextro-linkedin-card.png`

## Export Notes

The HTML manuscript is designed to be rendered to PDF with a browser-based print pipeline.

Recommended approach:

1. Serve the repository root locally with any static file server so relative assets resolve cleanly.
2. Open `http://127.0.0.1:8008/docs/publication/contextro-paper.html` in a modern browser.
3. Use print-to-PDF with background graphics enabled and **Headers and footers disabled**.
4. Save the output as `contextro-paper.pdf`.

Avoid printing directly from a `file:///...` URL with browser headers enabled. That path is what causes browser-generated timestamps and local file paths to appear in the PDF margins.
