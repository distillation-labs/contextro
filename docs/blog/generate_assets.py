"""Generate all blog assets from experiment results."""

import json
import numpy as np
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import matplotlib.patches as mpatches
from matplotlib.patches import FancyArrowPatch
import seaborn as sns
from pathlib import Path

RESULTS = Path(__file__).parent.parent.parent / "scripts/experiment_results/results.json"
OUT = Path(__file__).parent / "assets"
OUT.mkdir(exist_ok=True)

# ── Style ──────────────────────────────────────────────────────────────────────
BRAND   = "#1a1a2e"   # dark navy
ACCENT  = "#e94560"   # red
MCP_COL = "#0f3460"   # deep blue
CTRL_COL= "#e94560"   # red
BG      = "#f8f9fa"
GRID    = "#e0e0e0"

plt.rcParams.update({
    "font.family": "DejaVu Sans",
    "axes.facecolor": BG,
    "figure.facecolor": "white",
    "axes.spines.top": False,
    "axes.spines.right": False,
    "axes.grid": True,
    "grid.color": GRID,
    "grid.linewidth": 0.6,
    "axes.labelsize": 11,
    "axes.titlesize": 13,
    "axes.titleweight": "bold",
    "xtick.labelsize": 10,
    "ytick.labelsize": 10,
})

data = json.loads(RESULTS.read_text())
ctrl = {r["task_id"]: r for r in data if r["arm"] == "control"}
mcp  = {r["task_id"]: r for r in data if r["arm"] == "mcp"}

# ── 1. Headline metrics bar chart ──────────────────────────────────────────────
fig, axes = plt.subplots(1, 3, figsize=(14, 5))
fig.suptitle("Contextro MCP vs No-MCP: Headline Metrics\n8,496-file Production Codebase · 60 Tasks",
             fontsize=14, fontweight="bold", y=1.02)

ctrl_vals = [r for r in ctrl.values() if r["completed"]]
mcp_vals  = [r for r in mcp.values()  if r["completed"]]

metrics = [
    ("Total Tokens", sum(r["tokens_estimate"] for r in ctrl_vals),
                     sum(r["tokens_estimate"] for r in mcp_vals), "tokens"),
    ("Median Latency (ms)",
     sorted(r["wall_clock_ms"] for r in ctrl_vals)[len(ctrl_vals)//2],
     sorted(r["wall_clock_ms"] for r in mcp_vals)[len(mcp_vals)//2], "ms"),
    ("Mean Tool Calls",
     sum(r["tool_calls"] for r in ctrl_vals) / len(ctrl_vals),
     sum(r["tool_calls"] for r in mcp_vals)  / len(mcp_vals), "calls"),
]

for ax, (title, c_val, m_val, unit) in zip(axes, metrics):
    bars = ax.bar(["No MCP", "MCP"], [c_val, m_val],
                  color=[CTRL_COL, MCP_COL], width=0.5, zorder=3)
    ax.set_title(title)
    ax.set_ylabel(unit)
    for bar, val in zip(bars, [c_val, m_val]):
        label = f"{val:,.0f}" if val > 100 else f"{val:.1f}"
        ax.text(bar.get_x() + bar.get_width()/2, bar.get_height() * 1.02,
                label, ha="center", va="bottom", fontweight="bold", fontsize=11)
    reduction = (1 - m_val / c_val) * 100 if c_val > 0 else 0
    ax.text(0.5, 0.92, f"↓ {reduction:.0f}% reduction",
            transform=ax.transAxes, ha="center", color=MCP_COL,
            fontsize=10, fontweight="bold")

plt.tight_layout()
plt.savefig(OUT / "01_headline_metrics.png", dpi=180, bbox_inches="tight")
plt.close()
print("✓ 01_headline_metrics.png")

# ── 2. Token reduction by category ────────────────────────────────────────────
categories = {}
for tid, r in ctrl.items():
    if not r["completed"] or r.get("error") == "no_equivalent":
        continue
    cat = r["category"]
    categories.setdefault(cat, {"ctrl": 0, "mcp": 0})
    categories[cat]["ctrl"] += r["tokens_estimate"]
    if tid in mcp:
        categories[cat]["mcp"] += mcp[tid]["tokens_estimate"]

# Only categories where control has tokens
cats = {k: v for k, v in categories.items() if v["ctrl"] > 0}
labels = [c.replace("_", " ").title() for c in cats]
ctrl_t = [v["ctrl"] for v in cats.values()]
mcp_t  = [v["mcp"]  for v in cats.values()]
reductions = [(1 - m/c)*100 for c, m in zip(ctrl_t, mcp_t)]

order = sorted(range(len(reductions)), key=lambda i: reductions[i], reverse=True)
labels   = [labels[i]   for i in order]
ctrl_t   = [ctrl_t[i]   for i in order]
mcp_t    = [mcp_t[i]    for i in order]
reductions = [reductions[i] for i in order]

x = np.arange(len(labels))
w = 0.35
fig, ax = plt.subplots(figsize=(13, 6))
b1 = ax.bar(x - w/2, ctrl_t, w, label="No MCP", color=CTRL_COL, zorder=3)
b2 = ax.bar(x + w/2, mcp_t,  w, label="MCP",    color=MCP_COL,  zorder=3)
ax.set_xticks(x)
ax.set_xticklabels(labels, rotation=20, ha="right")
ax.set_ylabel("Total Tokens")
ax.set_title("Token Consumption by Category: No-MCP vs MCP")
ax.legend()
for i, (bar, red) in enumerate(zip(b2, reductions)):
    ax.text(bar.get_x() + bar.get_width()/2, bar.get_height() + 200,
            f"{red:.0f}%↓", ha="center", va="bottom",
            fontsize=9, color=MCP_COL, fontweight="bold")
plt.tight_layout()
plt.savefig(OUT / "02_token_by_category.png", dpi=180, bbox_inches="tight")
plt.close()
print("✓ 02_token_by_category.png")

# ── 3. Per-tool latency heatmap ────────────────────────────────────────────────
tool_data = []
for tid, r in mcp.items():
    if r["completed"]:
        tool_data.append({
            "task": tid,
            "latency": r["wall_clock_ms"],
            "tokens": r["tokens_estimate"],
            "category": r["category"],
        })
tool_data.sort(key=lambda x: x["latency"])

tasks   = [d["task"] for d in tool_data]
latency = [d["latency"] for d in tool_data]
tokens  = [d["tokens"]  for d in tool_data]

fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(16, 12))

# Latency
colors_lat = [MCP_COL if l < 100 else ACCENT if l < 1000 else "#c0392b" for l in latency]
bars = ax1.barh(tasks, latency, color=colors_lat, zorder=3)
ax1.set_xlabel("Latency (ms, log scale)")
ax1.set_xscale("log")
ax1.set_title("MCP Tool Latency\n(all 60 tasks)")
ax1.axvline(10,   color=GRID, linestyle="--", linewidth=1, label="10ms")
ax1.axvline(100,  color=ACCENT, linestyle="--", linewidth=1, alpha=0.5, label="100ms")
ax1.axvline(1000, color="#c0392b", linestyle="--", linewidth=1, alpha=0.5, label="1s")
ax1.legend(fontsize=8)
for bar, val in zip(bars, latency):
    ax1.text(val * 1.05, bar.get_y() + bar.get_height()/2,
             f"{val:.1f}", va="center", fontsize=7)

# Tokens
colors_tok = [MCP_COL if t < 200 else ACCENT if t < 800 else "#c0392b" for t in tokens]
bars2 = ax2.barh(tasks, tokens, color=colors_tok, zorder=3)
ax2.set_xlabel("Tokens")
ax2.set_title("MCP Tool Token Cost\n(all 60 tasks)")
for bar, val in zip(bars2, tokens):
    ax2.text(val + 5, bar.get_y() + bar.get_height()/2,
             str(val), va="center", fontsize=7)

plt.suptitle("Per-Task MCP Performance Profile", fontsize=14, fontweight="bold")
plt.tight_layout()
plt.savefig(OUT / "03_per_tool_latency.png", dpi=180, bbox_inches="tight")
plt.close()
print("✓ 03_per_tool_latency.png")

# ── 4. Token savings scatter ───────────────────────────────────────────────────
paired = [(ctrl[tid]["tokens_estimate"], mcp[tid]["tokens_estimate"], tid)
          for tid in ctrl if tid in mcp
          and ctrl[tid]["completed"] and mcp[tid]["completed"]
          and ctrl[tid].get("error") != "no_equivalent"
          and ctrl[tid]["tokens_estimate"] > 0]

cx, mx, labels_s = zip(*paired)
fig, ax = plt.subplots(figsize=(9, 7))
sc = ax.scatter(cx, mx, c=MCP_COL, alpha=0.7, s=60, zorder=3)
# y=x line (no improvement)
lim = max(max(cx), max(mx)) * 1.05
ax.plot([0, lim], [0, lim], "--", color=GRID, linewidth=1.5, label="No improvement")
# 95% reduction line
ax.plot([0, lim], [0, lim * 0.05], "-", color=ACCENT, linewidth=1.5, alpha=0.7, label="95% reduction")
ax.set_xlabel("Control Tokens (no MCP)")
ax.set_ylabel("MCP Tokens")
ax.set_title("Token Efficiency: Every Task\n(below the line = MCP wins)")
ax.legend()
ax.set_xlim(0, lim)
ax.set_ylim(0, max(mx) * 1.2)
# Annotate a few outliers
for c, m, t in sorted(paired, key=lambda x: x[0], reverse=True)[:3]:
    ax.annotate(t, (c, m), textcoords="offset points", xytext=(5, 5), fontsize=8)
plt.tight_layout()
plt.savefig(OUT / "04_token_scatter.png", dpi=180, bbox_inches="tight")
plt.close()
print("✓ 04_token_scatter.png")

# ── 5. Architecture diagram ────────────────────────────────────────────────────
fig, ax = plt.subplots(figsize=(14, 8))
ax.set_xlim(0, 14)
ax.set_ylim(0, 8)
ax.axis("off")
ax.set_facecolor("white")
fig.patch.set_facecolor("white")

def box(ax, x, y, w, h, label, sublabel="", color=MCP_COL, fontsize=11):
    rect = plt.Rectangle((x, y), w, h, facecolor=color, edgecolor="white",
                          linewidth=2, zorder=3, alpha=0.92)
    ax.add_patch(rect)
    ax.text(x + w/2, y + h/2 + (0.15 if sublabel else 0), label,
            ha="center", va="center", color="white",
            fontsize=fontsize, fontweight="bold", zorder=4)
    if sublabel:
        ax.text(x + w/2, y + h/2 - 0.25, sublabel,
                ha="center", va="center", color="white", fontsize=8, zorder=4, alpha=0.85)

def arrow(ax, x1, y1, x2, y2):
    ax.annotate("", xy=(x2, y2), xytext=(x1, y1),
                arrowprops=dict(arrowstyle="->", color="#555", lw=1.8))

# Agent
box(ax, 0.3, 3.2, 2.2, 1.6, "AI Agent", "Claude / Cursor\nCopilot / Kiro", color="#2c3e50")
# MCP Protocol
box(ax, 3.2, 3.2, 2.2, 1.6, "MCP Protocol", "stdio / HTTP\nStreamable", color="#8e44ad")
# Contextro Server
box(ax, 6.2, 2.8, 2.2, 2.4, "Contextro\nServer", "35 tools\n<350MB RAM", color=MCP_COL)

# Engines
engine_y = [6.2, 6.2, 6.2, 0.4, 0.4]
engine_x = [0.3, 2.8, 5.3, 2.0, 7.8]
engines  = [
    ("Vector\nEngine", "LanceDB\npotion-code-16m"),
    ("BM25\nEngine", "Keyword\nExact match"),
    ("Graph\nEngine", "rustworkx\nO(1) lookups"),
    ("Git\nEngine", "Semantic\ncommit search"),
    ("Memory\nEngine", "Cross-session\nTTL store"),
]
for (ex, ey), (name, sub) in zip(zip(engine_x, engine_y), engines):
    box(ax, ex, ey, 2.2, 1.4, name, sub, color="#16a085", fontsize=9)

# Arrows: agent → mcp → server
arrow(ax, 2.5, 4.0, 3.2, 4.0)
arrow(ax, 5.4, 4.0, 6.2, 4.0)
# Server → engines
for ex, ey in zip(engine_x, engine_y):
    cx = ex + 1.1
    cy = ey + 1.4 if ey > 3 else ey + 1.4
    sx, sy = 7.3, 4.0
    arrow(ax, sx, sy, cx, cy)

ax.text(7.0, 7.7, "Contextro Architecture", ha="center", fontsize=16,
        fontweight="bold", color=BRAND)
ax.text(7.0, 7.3, "Single local process · No cloud · No API keys",
        ha="center", fontsize=10, color="#555")

plt.tight_layout()
plt.savefig(OUT / "05_architecture.png", dpi=180, bbox_inches="tight")
plt.close()
print("✓ 05_architecture.png")

# ── 6. Autoresearch loop diagram ───────────────────────────────────────────────
fig, ax = plt.subplots(figsize=(10, 10))
ax.set_xlim(-1.5, 1.5)
ax.set_ylim(-1.5, 1.5)
ax.axis("off")
fig.patch.set_facecolor("white")

steps = [
    "Establish\nBaseline",
    "Define\nTarget",
    "Hypothesis\nBacklog",
    "One-Variable\nExperiment",
    "Benchmark",
    "Keep or\nDiscard",
    "Log\nInsight",
    "Reassess\n& Compound",
]
n = len(steps)
angles = [2 * np.pi * i / n - np.pi/2 for i in range(n)]
r = 1.05
colors_loop = [MCP_COL, "#8e44ad", "#16a085", ACCENT,
               "#e67e22", "#27ae60", "#2980b9", "#c0392b"]

for i, (step, angle, col) in enumerate(zip(steps, angles, colors_loop)):
    x, y = r * np.cos(angle), r * np.sin(angle)
    circle = plt.Circle((x, y), 0.22, color=col, zorder=3)
    ax.add_patch(circle)
    ax.text(x, y, step, ha="center", va="center", color="white",
            fontsize=8.5, fontweight="bold", zorder=4)
    # Arrow to next
    next_angle = angles[(i + 1) % n]
    nx, ny = r * np.cos(next_angle), r * np.sin(next_angle)
    mid_angle = (angle + next_angle) / 2
    mx, my = (r + 0.05) * np.cos(mid_angle), (r + 0.05) * np.sin(mid_angle)
    dx = nx - x; dy = ny - y
    length = np.sqrt(dx**2 + dy**2)
    ax.annotate("", xy=(x + dx/length * 0.23, y + dy/length * 0.23),
                xytext=(x + dx/length * (-0.23 + length), y + dy/length * (-0.23 + length)),
                arrowprops=dict(arrowstyle="->", color="#aaa", lw=1.5))

ax.text(0, 0.12, "Autoresearch", ha="center", fontsize=13,
        fontweight="bold", color=BRAND)
ax.text(0, -0.12, "Loop", ha="center", fontsize=13,
        fontweight="bold", color=BRAND)
ax.text(0, -0.38, "Data decides.\nKeep only wins.", ha="center",
        fontsize=9, color="#555")

ax.set_title("Contextro Self-Improving Research Loop",
             fontsize=14, fontweight="bold", pad=20)
plt.tight_layout()
plt.savefig(OUT / "06_autoresearch_loop.png", dpi=180, bbox_inches="tight")
plt.close()
print("✓ 06_autoresearch_loop.png")

# ── 7. Experiment design diagram ──────────────────────────────────────────────
fig, ax = plt.subplots(figsize=(12, 6))
ax.set_xlim(0, 12)
ax.set_ylim(0, 6)
ax.axis("off")
fig.patch.set_facecolor("white")

# Shared task set
box(ax, 4.5, 4.5, 3.0, 1.0, "60 Tasks", "Same codebase · Same prompts", color="#2c3e50", fontsize=10)
# Two arms
box(ax, 0.5, 2.0, 3.5, 1.5, "Control Arm", "grep + file reads\nNo MCP", color=CTRL_COL, fontsize=10)
box(ax, 8.0, 2.0, 3.5, 1.5, "Treatment Arm", "Contextro MCP\n35 tools", color=MCP_COL, fontsize=10)
# Results
box(ax, 0.5, 0.2, 3.5, 1.2, "Baseline", "335,100 tokens\n471ms median", color="#c0392b", fontsize=9)
box(ax, 8.0, 0.2, 3.5, 1.2, "MCP Result", "14,327 tokens\n5.8ms median", color="#16a085", fontsize=9)
# Comparison
box(ax, 4.2, 0.2, 3.6, 1.2, "95.7% Reduction\n81× Faster", "", color=MCP_COL, fontsize=11)

# Arrows
arrow(ax, 6.0, 4.5, 2.25, 3.5)
arrow(ax, 6.0, 4.5, 9.75, 3.5)
arrow(ax, 2.25, 2.0, 2.25, 1.4)
arrow(ax, 9.75, 2.0, 9.75, 1.4)
arrow(ax, 4.0, 0.8, 4.2, 0.8)
arrow(ax, 11.5, 0.8, 11.8, 0.8)

ax.text(6.0, 5.7, "Controlled Experiment Design",
        ha="center", fontsize=14, fontweight="bold", color=BRAND)
ax.text(6.0, 5.3, "Paired comparison · Same codebase · 8,496 files",
        ha="center", fontsize=10, color="#555")

plt.tight_layout()
plt.savefig(OUT / "07_experiment_design.png", dpi=180, bbox_inches="tight")
plt.close()
print("✓ 07_experiment_design.png")

# ── 8. Autoresearch improvements table chart ───────────────────────────────────
metrics_before = [0.625, 378, 229, 12.4, 0.23, 2547]
metrics_after  = [1.000, 116,  43,  6.8, 0.46, 1043]
metric_labels  = ["Hybrid MRR", "Tokens/search", "Tokens/explain",
                  "Index time (s)", "Cache hit rate", "Workflow tokens"]
improvements   = [(a/b if b != 0 else 1) for a, b in zip(metrics_after, metrics_before)]
# For tokens/time, lower is better — invert
improvements[1] = metrics_before[1] / metrics_after[1]
improvements[2] = metrics_before[2] / metrics_after[2]
improvements[3] = metrics_before[3] / metrics_after[3]
improvements[5] = metrics_before[5] / metrics_after[5]

fig, ax = plt.subplots(figsize=(10, 5))
x = np.arange(len(metric_labels))
bars = ax.bar(x, improvements, color=[MCP_COL if v >= 1 else CTRL_COL for v in improvements],
              zorder=3, width=0.6)
ax.axhline(1.0, color=GRID, linewidth=1.5, linestyle="--", label="No change")
ax.set_xticks(x)
ax.set_xticklabels(metric_labels, rotation=15, ha="right")
ax.set_ylabel("Improvement multiplier (higher = better)")
ax.set_title("Autoresearch Loop: Measured Improvements\n(each bar = how much better after the loop)")
for bar, val, bl, al in zip(bars, improvements, metrics_before, metrics_after):
    ax.text(bar.get_x() + bar.get_width()/2, bar.get_height() + 0.03,
            f"{val:.1f}×", ha="center", va="bottom", fontweight="bold", fontsize=10)
plt.tight_layout()
plt.savefig(OUT / "08_autoresearch_improvements.png", dpi=180, bbox_inches="tight")
plt.close()
print("✓ 08_autoresearch_improvements.png")

print(f"\nAll assets saved to: {OUT}")
print(f"Files: {sorted(p.name for p in OUT.glob('*.png'))}")
