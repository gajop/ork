#!/usr/bin/env python3
"""
Check that no source files exceed 500 lines of code.

Exit codes:
  0: All files are within the limit
  1: One or more files exceed the limit
"""
import argparse
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MAX_LOC = 500

EXCLUDE_DIRS = {".git", "target", "node_modules", "__pycache__", ".venv", "dist", "build"}
EXCLUDE_FILES = {".DS_Store"}

# File extensions to check
SOURCE_EXTENSIONS = {".rs", ".py", ".js", ".ts", ".jsx", ".tsx", ".yaml", ".yml"}


def count_lines(file_path: Path) -> int:
    """Count lines in a file, skipping empty lines and comments for more accurate LOC."""
    try:
        with open(file_path, "r", encoding="utf-8", errors="ignore") as f:
            return sum(1 for _ in f)
    except (OSError, UnicodeDecodeError):
        return 0


def iter_source_files(root_dir: Path) -> list[Path]:
    """Find all source files in the repository."""
    files: list[Path] = []
    for root, dirs, filenames in os.walk(root_dir):
        # Filter out excluded directories
        dirs[:] = [d for d in dirs if d not in EXCLUDE_DIRS]

        for name in filenames:
            if name in EXCLUDE_FILES:
                continue

            path = Path(root) / name

            # Check if file has a source extension
            if path.suffix in SOURCE_EXTENSIONS:
                files.append(path)

    return files


def check_file_loc(file_path: Path, max_loc: int = MAX_LOC) -> tuple[bool, int]:
    """
    Check if a file exceeds the max LOC limit.

    Returns:
        (is_ok, line_count) where is_ok is True if file is within limit
    """
    line_count = count_lines(file_path)
    return line_count <= max_loc, line_count


def main() -> int:
    parser = argparse.ArgumentParser(
        description=f"Check that no source files exceed {MAX_LOC} lines of code."
    )
    parser.add_argument(
        "--max-loc",
        type=int,
        default=MAX_LOC,
        help=f"Maximum lines of code per file (default: {MAX_LOC})",
    )
    parser.add_argument(
        "--verbose",
        "-v",
        action="store_true",
        help="Show all files checked, not just violations",
    )
    args = parser.parse_args()

    violations: list[tuple[Path, int]] = []
    all_files: list[tuple[Path, int]] = []

    source_files = iter_source_files(ROOT)

    for file_path in sorted(source_files):
        is_ok, line_count = check_file_loc(file_path, args.max_loc)
        rel_path = file_path.relative_to(ROOT)

        all_files.append((rel_path, line_count))

        if not is_ok:
            violations.append((rel_path, line_count))

    if args.verbose:
        print(f"\n{'File':<60} {'Lines':>6}")
        print("-" * 67)
        for path, lines in all_files:
            marker = "❌" if lines > args.max_loc else "✓"
            print(f"{marker} {str(path):<58} {lines:>6}")
        print(f"\nTotal files checked: {len(all_files)}")

    if violations:
        print(f"\n❌ Found {len(violations)} file(s) exceeding {args.max_loc} lines:\n")
        print(f"{'File':<60} {'Lines':>6} {'Over':>6}")
        print("-" * 73)
        for path, lines in violations:
            over = lines - args.max_loc
            print(f"{str(path):<60} {lines:>6} (+{over})")
        print(f"\nPlease refactor these files to be under {args.max_loc} lines.")
        return 1

    print(f"✓ All {len(all_files)} source files are within {args.max_loc} line limit.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
