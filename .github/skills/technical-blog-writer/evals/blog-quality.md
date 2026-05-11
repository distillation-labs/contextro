# Technical Blog Writer — Evals

## Eval 1: Structure Compliance

**Input:** "Write a blog post about our new search algorithm that achieves 1.000 MRR"

**Pass criteria:**
- [ ] Has TL;DR section within first 100 words
- [ ] Has "Problem" or equivalent section
- [ ] Has "Results" section with actual numbers
- [ ] Has "Limitations" section
- [ ] Has "Methodology" or "How we tested" section
- [ ] Total structure follows the 10-section template (at least 7 of 10 present)

**Fail if:** Missing TL;DR, missing limitations, or no data in results section.

---

## Eval 2: Tone and Accessibility

**Input:** "Write a blog post explaining our hybrid search fusion algorithm"

**Pass criteria:**
- [ ] Uses "we" not "I"
- [ ] Active voice in >80% of sentences
- [ ] No unexplained jargon (every technical term defined on first use)
- [ ] No marketing superlatives ("revolutionary", "game-changing", "groundbreaking")
- [ ] Paragraphs average ≤4 sentences
- [ ] A developer unfamiliar with the project can follow the narrative

**Fail if:** Uses passive voice predominantly, contains unexplained acronyms, or reads like marketing copy.

---

## Eval 3: Data Integrity

**Input:** "Write a blog post about our benchmark results showing 98% token reduction"

**Pass criteria:**
- [ ] Every performance claim has a specific number
- [ ] Numbers have units (ms, tokens, files, %)
- [ ] Comparison uses tables (not just prose)
- [ ] Source of data mentioned (benchmark name, codebase size, task count)
- [ ] No vague claims ("fast", "efficient", "better") without quantification

**Fail if:** Contains unquantified performance claims or numbers without context.

---

## Eval 4: Visual Placeholders

**Input:** "Write a blog post with charts showing our experiment results"

**Pass criteria:**
- [ ] Uses `[PLACEHOLDER: description]` for visuals during drafting
- [ ] Each placeholder has a descriptive label (not just "[PLACEHOLDER: chart]")
- [ ] Specifies chart type in placeholder (bar chart, scatter, architecture diagram)
- [ ] References SVG format, not PNG
- [ ] At least 3 visual placeholders for a data-heavy post

**Fail if:** Uses no placeholders, or specifies PNG/matplotlib for visuals.

---

## Eval 5: Honesty and Limitations

**Input:** "Write a blog post about our dead code detection tool"

**Pass criteria:**
- [ ] Limitations section present and specific (not generic disclaimers)
- [ ] Mentions at least one concrete failure case or known issue
- [ ] Does not overstate capabilities
- [ ] Acknowledges what was NOT tested
- [ ] Uses phrases like "we found", "our data shows" (not "it is proven")

**Fail if:** No limitations section, or limitations are vague/generic.

---

## Eval 6: Reproducibility

**Input:** "Write a blog post about our controlled experiment comparing MCP vs no-MCP"

**Pass criteria:**
- [ ] Methodology section describes how to reproduce the experiment
- [ ] Configuration/environment details included (or referenced)
- [ ] Statistical approach mentioned (sample size, comparison method)
- [ ] Raw data availability mentioned (appendix, repo link, or inline)
- [ ] Another researcher could replicate from the description alone

**Fail if:** Methodology is hand-wavy or results cannot be independently verified.

---

## Eval 7: Opening Hook

**Input:** "Write a blog post about reducing AI agent token costs by 98%"

**Pass criteria:**
- [ ] First paragraph captures attention without clickbait
- [ ] TL;DR is under 50 words
- [ ] The "so what" is clear within 3 sentences
- [ ] No throat-clearing ("In this blog post, we will discuss...")
- [ ] Leads with the result, not the process

**Fail if:** Opens with meta-commentary about the post itself, or buries the lede.
