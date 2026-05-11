# SVG Diagram Engineer — Evals

## Eval 1: Valid SVG Output

**Input:** "Create an architecture diagram showing Agent → MCP → Server → Engines"

**Pass criteria:**
- [ ] Output is valid SVG (starts with `<svg xmlns=...>`)
- [ ] Has `viewBox` attribute (not fixed width/height in pixels)
- [ ] Has `font-family` with system font stack
- [ ] Contains `<text>` elements for all labels
- [ ] File renders in a browser without errors

**Fail if:** Output is not valid SVG, uses pixel dimensions, or requires external resources.

---

## Eval 2: Color Palette Compliance

**Input:** "Create a bar chart comparing control vs treatment results"

**Pass criteria:**
- [ ] Uses ≤5 distinct colors
- [ ] Primary/positive elements use navy (#0f3460) or similar dark blue
- [ ] Negative/control elements use red (#dc2626) or similar
- [ ] Text uses dark (#1a1a2e) for primary, gray (#6b7280) for secondary
- [ ] Background is light (#fafafa or #ffffff)
- [ ] No gradients on data elements

**Fail if:** Uses rainbow colors, gradients on bars, or dark backgrounds.

---

## Eval 3: Typography Hierarchy

**Input:** "Create a diagram with title, subtitle, labels, and annotations"

**Pass criteria:**
- [ ] Title: font-size ≥18, font-weight 700
- [ ] Subtitle: font-size 12-13, lighter color than title
- [ ] Labels: font-size 11-13, readable at 50% zoom
- [ ] No text smaller than font-size="10"
- [ ] text-anchor="middle" used for centered elements
- [ ] Consistent font-size within the same hierarchy level

**Fail if:** Text is unreadable at normal zoom, or hierarchy is unclear.

---

## Eval 4: No Chartjunk

**Input:** "Create a data visualization showing token savings by category"

**Pass criteria:**
- [ ] No 3D effects
- [ ] No decorative borders or ornaments
- [ ] No background patterns or textures
- [ ] Grid lines (if present) are subtle (#e5e7eb or lighter)
- [ ] Every visual element conveys data or aids navigation
- [ ] Whitespace used intentionally for readability

**Fail if:** Contains decorative elements that don't represent data.

---

## Eval 5: Proper Sizing

**Input:** "Create a flow diagram for an experiment design"

**Pass criteria:**
- [ ] viewBox dimensions match the diagram type guidelines
- [ ] Elements don't overlap or clip
- [ ] Adequate padding from edges (≥40px equivalent)
- [ ] Text doesn't extend beyond container boxes
- [ ] Arrows/lines don't cross text

**Fail if:** Elements overlap, text is clipped, or layout is cramped.

---

## Eval 6: Accessibility

**Input:** "Create a comparison chart with two data series"

**Pass criteria:**
- [ ] Color is not the only differentiator (labels also distinguish series)
- [ ] Sufficient contrast between text and background
- [ ] Legend present when multiple series exist
- [ ] All data values have text labels (not just visual encoding)
- [ ] Diagram is understandable in grayscale

**Fail if:** Removing color makes the diagram incomprehensible.

---

## Eval 7: Consistency Across a Set

**Input:** "Create 3 diagrams for the same blog post: architecture, results, improvements"

**Pass criteria:**
- [ ] Same font-family across all three
- [ ] Same color palette across all three
- [ ] Same title/subtitle styling across all three
- [ ] Same background color across all three
- [ ] Same shadow filter definition across all three
- [ ] Visually cohesive as a set

**Fail if:** Diagrams look like they came from different tools or designers.

---

## Eval 8: SVG Not PNG

**Input:** "Generate a chart for our blog post"

**Pass criteria:**
- [ ] Output is SVG, not a Python script that generates PNG
- [ ] Does not import matplotlib, seaborn, or plotly
- [ ] Does not reference image generation APIs
- [ ] SVG is self-contained (no external CSS or fonts)
- [ ] Can be embedded directly in HTML or Markdown

**Fail if:** Generates a script instead of an SVG, or produces raster output.

---

## Eval 9: Data Accuracy

**Input:** "Create a bar chart: Control=335,100 tokens, MCP=14,327 tokens"

**Pass criteria:**
- [ ] Bar heights are proportional to values
- [ ] Exact values appear as text labels
- [ ] Reduction percentage is calculated correctly (95.7%)
- [ ] No visual distortion (bars start from same baseline)
- [ ] Units are labeled

**Fail if:** Bar proportions don't match data, or calculated percentages are wrong.

---

## Eval 10: Editability

**Input:** "Create an architecture diagram I can modify later"

**Pass criteria:**
- [ ] SVG uses semantic grouping (related elements near each other in source)
- [ ] Colors defined as hex values (easy to find-replace)
- [ ] Text content is plain text (not paths)
- [ ] No minification — human-readable source
- [ ] Comments or clear structure for major sections

**Fail if:** SVG is minified, text is converted to paths, or structure is incomprehensible.
