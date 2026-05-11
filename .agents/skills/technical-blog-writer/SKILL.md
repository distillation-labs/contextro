---
name: technical-blog-writer
description: >
  Write publication-quality technical blog posts with a senior researcher tone that remains
  accessible to all readers. Trigger when the user asks to write a blog post, research paper,
  technical announcement, product launch post, or any long-form content meant for public
  consumption. Do not use for internal docs, code comments, or quick summaries.
when_to_use: >
  Use when producing blog posts, research write-ups, launch announcements, or any content
  that will be read by developers, researchers, or the public. Especially useful when the
  content involves benchmarks, experiments, architecture explanations, or product narratives.
metadata:
  version: "0.1.0"
  category: content
  tags: [blog, writing, research, communication, technical-writing]
license: Proprietary
---

# Technical Blog Writer

Write like a senior researcher explaining their work to a curious colleague.
Professional. Clear. No gatekeeping.

## Principles

1. **Senior researcher tone, junior-friendly language.** Write with authority but never
   condescension. If a concept needs explaining, explain it. Don't assume the reader knows
   your stack.

2. **Show the work.** Include methodology, data, and honest limitations. Readers trust
   transparency more than polish.

3. **Lead with the result.** TL;DR at the top. The busy reader gets the conclusion in 10
   seconds. The curious reader gets the full story below.

4. **Data over claims.** Every performance claim has a number. Every number has a source.
   "Fast" means nothing. "2ms median latency on 8,496 files" means everything.

5. **No gatekeeping.** Open source the code. Publish the benchmarks. Share the methodology.
   Knowledge shared is knowledge multiplied.

## Structure

```
1. TL;DR (3-4 sentences, the entire story compressed)
2. The Problem (why this matters, what's broken)
3. What We Built (capabilities, not implementation details)
4. Research Methodology (how we tested, controlled variables)
5. Results (data tables, charts, honest numbers)
6. How We Achieved This (technical depth for those who want it)
7. Limitations (what doesn't work, what we're still fixing)
8. What's Next (future work, open questions)
9. Conclusion (one paragraph, the takeaway)
10. Appendix (raw data, configuration, reproducibility details)
```

## Tone Rules

- Use "we" not "I" — it's a team effort even if one person wrote it.
- Active voice. "We ran 60 tasks" not "60 tasks were run."
- Short sentences for claims. Long sentences for explanations.
- No marketing superlatives. No "revolutionary" or "game-changing."
- Concrete examples over abstract descriptions.
- When comparing, use tables. Humans parse tables faster than prose.
- Acknowledge prior art. Credit what inspired you.

## Formatting

- Markdown only. No proprietary formats.
- Use `[PLACEHOLDER: description]` for visuals not yet created.
- Tables for comparisons. Code blocks for examples. Headers for navigation.
- Keep paragraphs to 3-4 sentences max.
- One idea per paragraph.

## Visuals

- All diagrams as SVG (see `svg-diagram-engineer` skill).
- Data charts as SVG with clean, minimal design.
- Every chart has a title, axis labels, and a one-line caption.
- Use `[PLACEHOLDER: description]` during drafting, replace with SVG paths in final.

## Quality Checklist

Before publishing, verify:

- [ ] TL;DR captures the full story in under 50 words
- [ ] Every claim has a supporting number or citation
- [ ] Limitations section is honest and specific
- [ ] A developer unfamiliar with the project can follow the narrative
- [ ] No jargon without explanation on first use
- [ ] All placeholders replaced with actual content
- [ ] Methodology is reproducible from the description alone
- [ ] Code examples are complete and runnable
- [ ] Install/usage instructions tested on a clean environment

## Anti-Patterns

- Do not write walls of text without structure.
- Do not hide bad results. Report them and explain why.
- Do not use passive voice to obscure who did what.
- Do not assume the reader has context from previous posts.
- Do not use screenshots of terminals — use code blocks.
- Do not publish without a "Limitations" section.
- Do not use PNG for diagrams when SVG is available.
