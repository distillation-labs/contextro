---
name: docs-maintainer
description: >
  Use for writing and maintaining product documentation and release-facing docs:
  changelogs, README updates, release notes, version/tag docs, publication manifests,
  installation and usage docs, and cross-doc sync.
when_to_use: >
  Use when docs drift from code, when a release or tag needs documentation support,
  when changelog or README content must be updated, or when multiple docs need a
  consistent maintenance pass.
metadata:
  version: "1.0.0"
  category: docs-ops
  tags: [docs, changelog, readme, release-notes, publication, release, sync]
license: Proprietary
---

# Docs Maintainer

Maintain the documentation surface that users and future agents rely on.

## Use It For

- Changelog entries
- README and install docs
- Release notes and tag docs
- Publication manifests and launch docs
- Removing stale paths or broken links

## Do Not Use It For

- Product implementation
- Broad rewrite work when a small patch is enough
- Claims that cannot be supported by repo state

## Core Rules

- Keep docs aligned with shipped code.
- Keep changes small and factual.
- Update dependent docs together.
- Remove stale references after file moves or deletions.

## References

- `references/docs-maintenance-patterns.md`
- `references/eval-rubric.md`
