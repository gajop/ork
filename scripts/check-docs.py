#!/usr/bin/env python3
import argparse
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CRATES_DIR = ROOT / "crates"
DOCS_DIR = ROOT / "docs" / "dev" / "crates"

EXCLUDE_DIRS = {".git", "target", "node_modules", "__pycache__", ".venv"}
EXCLUDE_FILES = {".DS_Store"}

LINK_RE = re.compile(r"\[(?P<text>[^\]]+)\]\((?P<link>[^)]+)\)")


def run_git(args):
    return subprocess.run(
        ["git", *args],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )


def file_metadata(path: Path) -> tuple[str, str]:
    updated = ""
    try:
        mtime = path.stat().st_mtime
        updated = (
            __import__("datetime")
            .datetime.fromtimestamp(mtime, __import__("datetime").timezone.utc)
            .date()
            .isoformat()
        )
    except OSError:
        updated = "unknown"

    rel = path.relative_to(ROOT).as_posix()
    res = run_git(["hash-object", rel])
    if res.returncode != 0 or not res.stdout.strip():
        return (updated, "unknown")
    return (updated, res.stdout.strip()[:7])


def iter_crate_files(crate_dir: Path) -> set[str]:
    files: set[str] = set()
    for root, dirs, filenames in os.walk(crate_dir):
        dirs[:] = [d for d in dirs if d not in EXCLUDE_DIRS]
        for name in filenames:
            if name in EXCLUDE_FILES:
                continue
            path = Path(root) / name
            rel = path.relative_to(crate_dir).as_posix()
            files.add(rel)
    return files


def parse_files_table(doc_path: Path) -> tuple[list[str], int, int, list[dict]]:
    lines = doc_path.read_text().splitlines()
    start = None
    for idx, line in enumerate(lines):
        if line.strip() == "## Files":
            start = idx
            break
    if start is None:
        raise ValueError(f"Missing '## Files' section in {doc_path}")

    header_idx = None
    for idx in range(start + 1, len(lines)):
        if lines[idx].lstrip().startswith("|"):
            header_idx = idx
            break
    if header_idx is None:
        raise ValueError(f"Missing files table in {doc_path}")

    sep_idx = header_idx + 1
    row_start = header_idx + 2
    row_end = row_start
    while row_end < len(lines) and lines[row_end].lstrip().startswith("|"):
        row_end += 1

    header_cells = [c.strip() for c in lines[header_idx].strip().strip("|").split("|")]
    if header_cells != ["File", "Purpose", "Updated", "File SHA"]:
        raise ValueError(
            f"Unexpected table header in {doc_path}: {header_cells} (expected File | Purpose | Updated | File SHA)"
        )

    rows: list[dict] = []
    for line in lines[row_start:row_end]:
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) != 4:
            raise ValueError(f"Bad table row in {doc_path}: {line}")
        file_cell, purpose_cell, updated_cell, commit_cell = cells
        match = LINK_RE.match(file_cell)
        if not match:
            raise ValueError(f"File cell is not a link in {doc_path}: {file_cell}")
        rows.append(
            {
                "line": line,
                "file_cell": file_cell,
                "file_text": match.group("text"),
                "file_link": match.group("link"),
                "purpose": purpose_cell,
                "updated": updated_cell,
                "commit": commit_cell,
            }
        )

    return lines, row_start, row_end, rows


def update_doc(doc_path: Path, check_only: bool) -> tuple[bool, list[str]]:
    crate_name = doc_path.stem
    crate_dir = CRATES_DIR / crate_name
    if not crate_dir.exists():
        return False, [f"Missing crate directory for {crate_name}: {crate_dir}"]

    errors: list[str] = []
    lines, row_start, row_end, rows = parse_files_table(doc_path)

    doc_files: set[str] = set()
    updated_rows: list[tuple[str, str, str, str]] = []

    for row in rows:
        link_target = (doc_path.parent / row["file_link"]).resolve()
        if not link_target.exists():
            errors.append(f"{doc_path}: missing file {row['file_link']}")
            updated_rows.append((row["file_cell"], row["purpose"], row["updated"], row["commit"]))
            continue
        if not link_target.is_file():
            errors.append(f"{doc_path}: link is not a file {row['file_link']}")
            updated_rows.append((row["file_cell"], row["purpose"], row["updated"], row["commit"]))
            continue

        try:
            rel_from_crate = link_target.relative_to(crate_dir).as_posix()
        except ValueError:
            errors.append(f"{doc_path}: link not under crate {row['file_link']}")
            updated_rows.append((row["file_cell"], row["purpose"], row["updated"], row["commit"]))
            continue

        if rel_from_crate != row["file_text"]:
            errors.append(
                f"{doc_path}: link text '{row['file_text']}' does not match path '{rel_from_crate}'"
            )

        doc_files.add(rel_from_crate)
        updated, file_sha = file_metadata(link_target)
        updated_rows.append((row["file_cell"], row["purpose"], updated, file_sha))

    crate_files = iter_crate_files(crate_dir)
    missing_docs = sorted(crate_files - doc_files)
    missing_files = sorted(doc_files - crate_files)

    if missing_docs:
        errors.append(
            f"{doc_path}: missing docs for files: {', '.join(missing_docs)}"
        )
    if missing_files:
        errors.append(
            f"{doc_path}: docs reference missing files: {', '.join(missing_files)}"
        )

    sorted_rows = sorted(
        updated_rows,
        key=lambda item: (
            item[0].split("](")[-1].rstrip(")")  # link path from file cell
        ),
    )

    if check_only and updated_rows != sorted_rows:
        errors.append(f"{doc_path}: files table is not sorted by full path")

    if not check_only:
        updated_lines = [
            f"| {file_cell} | {purpose} | {updated} | {file_sha} |"
            for file_cell, purpose, updated, file_sha in sorted_rows
        ]
        new_lines = lines[:row_start] + updated_lines + lines[row_end:]
        new_text = "\n".join(new_lines) + "\n"
        if new_text != doc_path.read_text():
            doc_path.write_text(new_text)

    return not errors, errors


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Validate crate docs file tables and update Updated/Commit columns."
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Only check for issues; do not write updates.",
    )
    args = parser.parse_args()

    errors: list[str] = []

    crate_dirs = [d for d in CRATES_DIR.iterdir() if d.is_dir()]
    for crate_dir in sorted(crate_dirs):
        doc_path = DOCS_DIR / f"{crate_dir.name}.md"
        if not doc_path.exists():
            errors.append(f"Missing docs file for crate {crate_dir.name}: {doc_path}")

    for doc_path in sorted(DOCS_DIR.glob("*.md")):
        ok, doc_errors = update_doc(doc_path, args.check)
        if not ok:
            errors.extend(doc_errors)

    if errors:
        for err in errors:
            print(err)
        return 1

    print("Docs tables are in sync.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
