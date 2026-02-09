#!/usr/bin/env python3
import argparse
import subprocess
import sys
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUTPUT = ROOT / "docs" / "dev" / "file-review.md"


@dataclass
class ExistingRow:
    sha: str
    review_comment: str
    valid: str


@dataclass
class FileMeta:
    modified: str
    sha: str


def main() -> int:
    args = parse_args()
    output_path = resolve_output_path(args.output)
    existing_rows = parse_existing_rows(output_path)
    metadata, crate_sections, other_files = collect_metadata(output_path)
    new_text = build_markdown(metadata, crate_sections, other_files, existing_rows)

    old_text = output_path.read_text() if output_path.exists() else ""
    if old_text == new_text:
        print("Review tracker is in sync.")
        return 0

    if args.check:
        print(f"{output_path}: review tracker is out of date")
        return 1

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(new_text)
    print(f"Updated {output_path}")
    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Generate/update a markdown tracker for human file review."
    )
    parser.add_argument(
        "--output",
        default=str(DEFAULT_OUTPUT.relative_to(ROOT)),
        help=f"Output markdown path (default: {DEFAULT_OUTPUT.relative_to(ROOT)})",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Check whether output is up to date without writing.",
    )
    return parser.parse_args()


def resolve_output_path(arg_value: str) -> Path:
    candidate = Path(arg_value)
    if candidate.is_absolute():
        return candidate
    return ROOT / candidate


def build_markdown(
    metadata: dict[str, FileMeta],
    crate_sections: dict[str, list[str]],
    other_files: list[str],
    existing_rows: dict[str, ExistingRow],
) -> str:
    lines: list[str] = []
    lines.append("# File Review Tracker")
    lines.append("")
    lines.append("This file is maintained by `scripts/check-review.py`.")
    lines.append("Only edit the `Review comment` and `Valid` columns.")
    lines.append("")

    def add_section(title: str, files: list[str]) -> None:
        lines.append(f"## {title}")
        lines.append("")
        lines.append("| File | Last Modified (git) | File SHA | Review comment | Valid |")
        lines.append("| --- | --- | --- | --- | --- |")
        for path in files:
            current = metadata[path]
            previous = existing_rows.get(path)
            review_comment = previous.review_comment if previous else ""
            valid = valid_value(previous, current.sha)
            lines.append(
                f"| `{path}` | {current.modified} | {current.sha} | {review_comment} | {valid} |"
            )
        lines.append("")

    for crate in sorted(crate_sections):
        add_section(f"crate: {crate}", crate_sections[crate])

    add_section("non-crate files", sorted(other_files))
    return "\n".join(lines).rstrip() + "\n"


def collect_metadata(output_path: Path) -> tuple[dict[str, FileMeta], dict[str, list[str]], list[str]]:
    try:
        output_rel = output_path.resolve().relative_to(ROOT).as_posix()
    except ValueError:
        output_rel = None

    metadata: dict[str, FileMeta] = {}
    crate_sections: dict[str, list[str]] = defaultdict(list)
    other_files: list[str] = []

    for path in sorted(git_tracked_files()):
        if output_rel and path == output_rel:
            continue

        metadata[path] = FileMeta(
            modified=git_file_modified(path),
            sha=git_file_sha(path),
        )

        parts = PurePosixPath(path).parts
        if len(parts) >= 2 and parts[0] == "crates":
            crate_sections[parts[1]].append(path)
        else:
            other_files.append(path)

    return metadata, crate_sections, other_files


def parse_existing_rows(output_path: Path) -> dict[str, ExistingRow]:
    if not output_path.exists():
        return {}

    rows: dict[str, ExistingRow] = {}
    for line in output_path.read_text().splitlines():
        stripped = line.strip()
        if not (stripped.startswith("|") and stripped.endswith("|")):
            continue
        cells = [cell.strip() for cell in stripped.strip("|").split("|")]
        if len(cells) < 5:
            continue

        file_path = parse_file_cell(cells[0])
        if file_path is None:
            continue

        review_comment = "|".join(cells[3:-1]).strip()
        rows[file_path] = ExistingRow(
            sha=cells[2].strip(),
            review_comment=review_comment,
            valid=cells[-1].strip(),
        )
    return rows


def valid_value(existing: ExistingRow | None, current_sha: str) -> str:
    if existing is None:
        return "OUTDATED (new)"

    if existing.sha == current_sha:
        return existing.valid

    if existing.valid.startswith("OUTDATED"):
        return existing.valid

    previous = existing.sha if existing.sha else "unknown"
    return f"OUTDATED (prev: {previous})"


def parse_file_cell(file_cell: str) -> str | None:
    value = file_cell.strip()
    if value.startswith("`") and value.endswith("`"):
        value = value[1:-1]

    if value.startswith("[") and "](" in value and value.endswith(")"):
        close_idx = value.find("](")
        if close_idx > 1:
            value = value[close_idx + 2 : -1]

    if not value or value == "File":
        return None
    if value.startswith("---"):
        return None
    return value


def git_tracked_files() -> list[str]:
    res = run_git(["ls-files", "-z"])
    if res.returncode != 0:
        raise RuntimeError(res.stderr.strip() or "git ls-files failed")
    return [path for path in res.stdout.split("\0") if path]


def git_file_modified(path: str) -> str:
    res = run_git(["log", "-1", "--format=%cs", "--", path])
    if res.returncode != 0:
        return "unknown"
    value = res.stdout.strip()
    return value if value else "uncommitted"


def git_file_sha(path: str) -> str:
    res = run_git(["hash-object", path])
    if res.returncode != 0:
        return "unknown"
    value = res.stdout.strip()
    return value[:7] if value else "unknown"


def run_git(args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )


if __name__ == "__main__":
    sys.exit(main())
