---
name: svg-diagram-engineer
description: >
  Generate publication-quality SVG diagrams for technical blogs, documentation, and
  presentations. Trigger when the user needs architecture diagrams, flow charts, data
  visualizations, comparison charts, or any visual that should be crisp, scalable, and
  professional. Do not use for raster images, photos, or illustrations that require
  generative AI art.
when_to_use: >
  Use when creating visuals for blog posts, documentation, presentations, or any context
  where clean vector graphics are needed. Especially useful for architecture diagrams,
  experiment designs, bar charts, scatter plots, flow diagrams, and data tables rendered
  as graphics.
metadata:
  version: "0.1.0"
  category: content
  tags: [svg, diagrams, visualization, design, charts]
license: Proprietary
---

# SVG Diagram Engineer

Create clean, professional SVG diagrams. No matplotlib. No chartjunk. Publication-ready.

## Principles

1. **SVG only.** Never generate PNG/JPEG for diagrams. SVGs are scalable, editable, small,
   and render perfectly at any resolution.

2. **Minimal design.** White/light background. Limited color palette. No gradients unless
   they serve a purpose. No 3D effects. No decorative elements.

3. **Typography first.** Use system fonts (Inter, -apple-system, sans-serif). Proper
   hierarchy: title > subtitle > labels > annotations. Never smaller than 10px.

4. **Color with purpose.** Each color means something. Use a consistent palette across
   all diagrams in a set. Maximum 5 colors per diagram.

5. **Data-ink ratio.** Every pixel of ink should represent data. Remove gridlines, borders,
   and decorations that don't convey information.

## Standard Palette

```
Primary (MCP/positive):  #0f3460 (deep navy)
Negative/control:        #dc2626 (red)
Success/improvement:     #059669 (green)
Accent/highlight:        #7c3aed (purple)
Engine/component:        #0d9488 (teal)
Text primary:            #1a1a2e
Text secondary:          #6b7280
Background:              #fafafa
Card background:         #ffffff
Border/grid:             #e5e7eb
```

## SVG Template

```xml
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 [width] [height]"
     font-family="Inter, -apple-system, system-ui, sans-serif">
  <defs>
    <filter id="shadow" x="-4%" y="-4%" width="108%" height="108%">
      <feDropShadow dx="0" dy="2" stdDeviation="4" flood-opacity="0.08"/>
    </filter>
  </defs>
  <rect width="[width]" height="[height]" fill="#fafafa"/>
  <!-- Title -->
  <text x="[center]" y="40" text-anchor="middle" font-size="20"
        font-weight="700" fill="#1a1a2e">[Title]</text>
  <text x="[center]" y="62" text-anchor="middle" font-size="12"
        fill="#6b7280">[Subtitle]</text>
  <!-- Content here -->
</svg>
```

## Diagram Types

### Architecture Diagram
- Boxes with rounded corners (rx="12"), subtle shadow
- Arrows as lines with marker-end arrowheads
- Left-to-right or top-to-bottom flow
- Group related components visually

### Bar Chart
- Vertical bars, rounded top (rx="3-4")
- Value labels above bars, not inside
- Category labels below
- Improvement badges as colored pills below pairs

### Scatter Plot
- Circles with opacity (0.7) for overlap visibility
- Color-code by category
- Reference lines (diagonal, threshold) as dashed
- Annotation box for key insight

### Flow Diagram
- Top-to-bottom or left-to-right
- Dashed arrows for optional/async connections
- Solid arrows for primary flow
- Result boxes at the bottom

### Data Table (as SVG)
- Clean rows with alternating subtle backgrounds or separator lines
- Bold headers, regular data
- Highlight column for key metric (green pills)

## Sizing Guidelines

| Diagram Type | Recommended viewBox |
|---|---|
| Architecture | 1200×700 |
| Bar chart | 1100×500 |
| Scatter plot | 800×600 |
| Flow diagram | 1100×550 |
| Data table | 1000×450 |
| Comparison (side-by-side) | 1100×480 |

## Quality Checklist

- [ ] Title and subtitle present
- [ ] All text readable at 50% zoom
- [ ] Colors from the standard palette
- [ ] No text smaller than 10px font-size
- [ ] Consistent spacing between elements
- [ ] Shadow filter applied to primary boxes
- [ ] viewBox set (not width/height in pixels)
- [ ] Renders correctly in browser, Notion, GitHub, and PDF
- [ ] File size under 10KB for simple diagrams, under 20KB for complex

## Anti-Patterns

- Do not use matplotlib, seaborn, or any raster-generating library for diagrams.
- Do not use gradients for bars or backgrounds.
- Do not use more than 5 colors in one diagram.
- Do not place text inside small boxes where it gets clipped.
- Do not use pixel dimensions — always use viewBox.
- Do not generate PNGs. If a PNG is needed, render the SVG via Chrome headless.
- Do not use decorative icons or clipart.
- Do not crowd the diagram — whitespace is a feature.
