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
- `figures/` - publication-specific figures used by the manuscript
- `social/` - platform copy and generated image assets

## Figure Assets

The paper uses a mix of publication-specific figures and existing project figures.

Publication-specific figures in `figures/`:

- `contextro-system-overview.svg`
- `contextro-study-design.svg`

Existing project figures reused from `docs/blog/assets/`:

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

The revised manuscript now distinguishes between three evidence layers:

- archived aggregate launch-era summaries in `scripts/experiment_results/summary.json`
- fresh paired-study aggregate rerun artifacts copied into this directory
- fresh public benchmark reruns on the public `src/` tree

The paired-study task catalogs are exported directly by `scripts/experiment_platform.py` and are designed to satisfy the paper checklist requirement for a machine-readable task inventory without disclosing private repository identifiers. `paired-study-tasks.json` preserves the full 60-task suite in sanitized form, while `paired-study-comparable-tasks.json` isolates the 39 tasks with scripted grep-plus-read control equivalents.

All refreshed token metrics in this package now use `tiktoken` with the `cl100k_base` encoding. The paired-study rerun config and public benchmark JSON files record that tokenizer metadata explicitly.

The robustness file uses 100 deterministic bootstrap task subsets over the 39 directly comparable tasks. The seed and sampling method are recorded inside `paired-study-subset-robustness.json`.

## Social Assets

Generated image previews currently live beside the SVG sources in `social/`:

- `contextro-social-hero.svg`
- `contextro-social-hero.png`
- `contextro-linkedin-card.svg`
- `contextro-linkedin-card.png`

## Export Notes

The HTML manuscript is designed to be rendered to PDF with a browser-based print pipeline.

Recommended approach:

1. Open `contextro-paper.html` in a modern browser.
2. Use print-to-PDF with background graphics enabled.
3. Save the output as `contextro-paper.pdf`.

If you want a fully automated export script, I can add one once you confirm the toolchain you want to standardize on.

## Task Catalog Export

To regenerate the machine-readable sanitized task lists without rerunning the full paired study:

1. Run `python scripts/experiment_platform.py --output-dir docs/publication --export-task-catalogs-only`
2. Inspect `paired-study-tasks.json` and `paired-study-comparable-tasks.json`
