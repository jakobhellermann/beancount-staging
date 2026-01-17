#!/usr/bin/env python3
# /// script
# dependencies = [
#   "matplotlib",
#   "pandas",
# ]
# ///
"""
Plot learning curves comparing different predictor strategies.

Automatically detects all strategies from CSV columns and plots them.

Usage:
    # Generate data
    cargo run --release -p beancount-staging-predictor --bin learning-curve -- \
        -j ~/finances/src/transactions.beancount \
        -j ~/finances/journal.beancount \
        -j ~/finances/src/ignored.beancount > learning_curve.csv

    # Plot all strategies
    uv run plot_learning_curve.py learning_curve.csv

    # Exclude specific strategies
    uv run plot_learning_curve.py learning_curve.csv --exclude dt_raw payee_freq
"""

import argparse
import sys

import matplotlib.pyplot as plt
import pandas as pd

parser = argparse.ArgumentParser(description="Plot learning curves from CSV data")
parser.add_argument("csv_file", help="Input CSV file with learning curve data")
parser.add_argument(
    "--exclude",
    nargs="+",
    help="Strategy names to exclude from plots (case-insensitive)",
    default=[],
)
args = parser.parse_args()

csv_file = args.csv_file
exclude_strategies = [s.lower() for s in args.exclude]

# Read the data
data = pd.read_csv(csv_file)

# Auto-detect strategies from CSV columns
# Expected format: {strategy_name}_accuracy and {strategy_name}_time_ms
accuracy_cols = [col for col in data.columns if col.endswith("_accuracy")]
time_cols = [col for col in data.columns if col.endswith("_time_ms")]

# Extract strategy names
strategies = []
for acc_col in accuracy_cols:
    strategy_name = acc_col.replace("_accuracy", "")
    time_col = f"{strategy_name}_time_ms"
    if time_col in time_cols:
        # Check if strategy should be excluded
        if strategy_name.lower() not in exclude_strategies:
            strategies.append(
                {
                    "name": strategy_name,
                    "accuracy_col": acc_col,
                    "time_col": time_col,
                }
            )

if not strategies:
    print(
        "Error: No strategies found in CSV (after exclusions). Expected columns like 'strategy_name_accuracy' and 'strategy_name_time_ms'"
    )
    sys.exit(1)

excluded_count = len(accuracy_cols) - len(strategies)
if excluded_count > 0:
    print(f"Excluded {excluded_count} strategies: {exclude_strategies}")
print(f"Plotting {len(strategies)} strategies: {[s['name'] for s in strategies]}")

# Define colors and markers (cycle if more strategies than defined)
colors = [
    "#e74c3c",
    "#2ecc71",
    "#3498db",
    "#9b59b6",
    "#f39c12",
    "#1abc9c",
    "#e67e22",
    "#34495e",
]
markers = ["o", "s", "^", "D", "v", "<", ">", "p"]

# Create figure with two subplots
fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(16, 6))

# Left plot: Accuracy
for i, strategy in enumerate(strategies):
    color = colors[i % len(colors)]
    marker = markers[i % len(markers)]

    # Format label with final accuracy at the beginning
    name = strategy["name"].replace("_", " ").title()
    final_acc = data.iloc[-1][strategy["accuracy_col"]]
    label = f"{final_acc:.1f}% - {name}"

    ax1.plot(
        data["training_size"],
        data[strategy["accuracy_col"]],
        marker=marker,
        linewidth=2,
        markersize=4,
        label=label,
        color=color,
    )

# Add horizontal line at 80% for reference
ax1.axhline(y=80, color="gray", linestyle="--", alpha=0.5, label="80% Target")

# Formatting left plot
ax1.set_xlabel("Training Examples", fontsize=12, fontweight="bold")
ax1.set_ylabel("Prediction Accuracy (%)", fontsize=12, fontweight="bold")
ax1.set_title("Accuracy vs Training Size", fontsize=13, fontweight="bold")
ax1.legend(fontsize=10, loc="lower right")
ax1.grid(True, alpha=0.3, linestyle="--")
ax1.set_xlim(0, max(data["training_size"]) + 100)
ax1.set_ylim(0, 100)

# Add annotations for final accuracy values
final_size = data.iloc[-1]["training_size"]
for i, strategy in enumerate(strategies):
    color = colors[i % len(colors)]
    final_acc = data.iloc[-1][strategy["accuracy_col"]]

    # Stagger annotation positions to avoid overlap
    offset_y = (i - len(strategies) / 2) * 3

    ax1.annotate(
        f"{final_acc:.1f}%",
        xy=(final_size, final_acc),
        xytext=(final_size - 200, final_acc + offset_y),
        fontsize=9,
        fontweight="bold",
        color=color,
        arrowprops=dict(arrowstyle="->", color=color, lw=1.5),
    )

# Right plot: Training time
for i, strategy in enumerate(strategies):
    color = colors[i % len(colors)]
    marker = markers[i % len(markers)]

    # Format label with final time at the beginning
    name = strategy["name"].replace("_", " ").title()
    final_time = data.iloc[-1][strategy["time_col"]]
    label = f"{final_time:.0f}ms - {name}"

    ax2.plot(
        data["training_size"],
        data[strategy["time_col"]],
        marker=marker,
        linewidth=2,
        markersize=4,
        label=label,
        color=color,
    )

# Formatting right plot
ax2.set_xlabel("Training Examples", fontsize=12, fontweight="bold")
ax2.set_ylabel("Training Time (ms)", fontsize=12, fontweight="bold")
ax2.set_title("Training Time vs Training Size", fontsize=13, fontweight="bold")
ax2.legend(fontsize=10, loc="upper left")
ax2.grid(True, alpha=0.3, linestyle="--")
ax2.set_xlim(0, max(data["training_size"]) + 100)

# Add annotations for final training time (skip if 0ms)
for i, strategy in enumerate(strategies):
    color = colors[i % len(colors)]
    final_time = data.iloc[-1][strategy["time_col"]]

    if final_time > 0:
        # Stagger annotation positions
        offset_y = (i - len(strategies) / 2) * 100

        ax2.annotate(
            f"{int(final_time)}ms",
            xy=(final_size, final_time),
            xytext=(final_size - 300, final_time + offset_y),
            fontsize=9,
            fontweight="bold",
            color=color,
            arrowprops=dict(arrowstyle="->", color=color, lw=1.5),
        )

# Overall title
fig.suptitle(
    "Learning Curves: Predictor Strategy Comparison",
    fontsize=15,
    fontweight="bold",
    y=0.98,
)

plt.tight_layout(rect=[0, 0, 1, 0.96])
output_file = csv_file.replace(".csv", ".png")
plt.savefig(output_file, dpi=150, bbox_inches="tight")
print(f"Graph saved to {output_file}")

# Print insights
print("\n=== Learning Curve Insights ===")

# Find best strategy at each key milestone
for milestone in [50, 80, 90, 95]:
    best_strategy = None
    best_size = float("inf")

    for strategy in strategies:
        crosses = data[data[strategy["accuracy_col"]] >= milestone]
        if not crosses.empty:
            size = crosses.iloc[0]["training_size"]
            if size < best_size:
                best_size = size
                best_strategy = strategy["name"].replace("_", " ").title()

    if best_strategy:
        print(
            f"First to reach {milestone}%: {best_strategy} at {int(best_size)} examples"
        )

print(f"\nFinal Accuracy (at {int(final_size)} examples):")
# Sort by accuracy descending
final_accuracies = [
    (s["name"].replace("_", " ").title(), data.iloc[-1][s["accuracy_col"]])
    for s in strategies
]
final_accuracies.sort(key=lambda x: x[1], reverse=True)

for name, acc in final_accuracies:
    print(f"  {name:30s}: {acc:.1f}%")

print(f"\nFinal Training Time:")
# Sort by time ascending
final_times = [
    (s["name"].replace("_", " ").title(), data.iloc[-1][s["time_col"]])
    for s in strategies
]
final_times.sort(key=lambda x: x[0])

for name, time in final_times:
    print(f"  {name:30s}: {int(time)}ms")

# Analyze trade-offs
if len(strategies) >= 2:
    best_acc = final_accuracies[0]
    fastest = min(final_times, key=lambda x: x[1])

    print(f"\nBest accuracy: {best_acc[0]} ({best_acc[1]:.1f}%)")
    print(f"Fastest training: {fastest[0]} ({int(fastest[1])}ms)")
