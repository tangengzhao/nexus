#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path
from typing import Iterable
from xml.sax.saxutils import escape
from zipfile import ZIP_DEFLATED, ZipFile


ROOT = Path(__file__).resolve().parent
CODECAT_RS = ROOT / "codecat.rs"
CODECAT_DOCX = ROOT / "codecat.docx"
DOCX_HEAD_LINES = 2000
DOCX_TAIL_LINES = 2000
DOCX_LINES_PER_PAGE = 50


def find_src_dirs() -> list[Path]:
    """Discover all src/ directories one level under ROOT (e.g. hsb-core/src/)."""
    src_dirs: list[Path] = []
    for child in sorted(ROOT.iterdir()):
        candidate = child / "src"
        if candidate.is_dir():
            src_dirs.append(candidate)
    # Also check ROOT/src itself if it exists
    root_src = ROOT / "src"
    if root_src.is_dir() and root_src not in src_dirs:
        src_dirs.insert(0, root_src)
    return src_dirs


def rust_source_files(src_dirs: list[Path]) -> list[tuple[str, Path]]:
    """Collect .rs files from all given src directories, ordered by crate then file.

    Returns list of (crate_name, path) tuples.
    """
    all_files: list[tuple[str, Path]] = []
    for src_dir in src_dirs:
        crate_name = src_dir.parent.name
        files = list(src_dir.rglob("*.rs"))
        main_rs = src_dir / "main.rs"
        rest = [path for path in files if path != main_rs]
        rest.sort(key=lambda path: (path.name.casefold(), path.relative_to(src_dir).as_posix().casefold()))
        ordered: list[Path] = []
        if main_rs.exists():
            ordered.append(main_rs)
        ordered.extend(rest)
        for path in ordered:
            all_files.append((crate_name, path))
    # Sort by crate name, then by relative path within that crate's src dir
    src_dir_map = {d: d.parent.name for d in src_dirs}
    path_to_src = {}
    for src_dir in src_dirs:
        for path in src_dir.rglob("*.rs"):
            path_to_src[path] = src_dir
    all_files.sort(
        key=lambda t: (
            t[0].casefold(),
            t[1].relative_to(path_to_src.get(t[1], t[1].parents[0])).as_posix().casefold(),
        )
    )
    return all_files


def non_empty_source_lines(entries: Iterable[tuple[str, Path]]) -> Iterable[tuple[str, str]]:
    """Yield (crate_name, non_empty_line) for each .rs file."""
    for crate_name, path in entries:
        text = path.read_text(encoding="utf-8", errors="replace")
        for line in text.splitlines():
            if line.strip():
                yield (crate_name, line)


def write_numbered_codecat(entries: Iterable[tuple[str, str]]) -> int:
    """Write numbered lines. Each entry is (crate_name, line_text)."""
    count = 0
    current_crate: str | None = None
    with CODECAT_RS.open("w", encoding="utf-8", newline="\n") as handle:
        for crate_name, line in entries:
            if crate_name != current_crate:
                current_crate = crate_name
                count += 1
                handle.write(f"{count}\t// ===== {crate_name} =====\n")
            count += 1
            handle.write(f"{count}\t{line}\n")
    return count


def docx_lines(numbered_lines: list[str]) -> list[str]:
    head = numbered_lines[:DOCX_HEAD_LINES]
    tail = numbered_lines[-DOCX_TAIL_LINES:] if len(numbered_lines) > DOCX_HEAD_LINES else []
    return head + tail


def paragraph_xml(text: str, page_break_after: bool) -> str:
    escaped_text = escape(text)
    paragraph = (
        "<w:p>"
        "<w:r><w:rPr><w:rFonts w:ascii=\"Consolas\" w:hAnsi=\"Consolas\"/>"
        "<w:sz w:val=\"16\"/></w:rPr>"
        f"<w:t xml:space=\"preserve\">{escaped_text}</w:t></w:r>"
    )
    if page_break_after:
        paragraph += "<w:r><w:br w:type=\"page\"/></w:r>"
    return paragraph + "</w:p>"


def build_document_xml(lines: list[str]) -> str:
    paragraphs = []
    for index, line in enumerate(lines, start=1):
        page_break_after = index % DOCX_LINES_PER_PAGE == 0 and index != len(lines)
        paragraphs.append(paragraph_xml(line, page_break_after))
    body = "".join(paragraphs)
    section_props = (
        "<w:sectPr>"
        "<w:pgSz w:w=\"11906\" w:h=\"16838\"/>"
        "<w:pgMar w:top=\"720\" w:right=\"720\" w:bottom=\"720\" w:left=\"720\" "
        "w:header=\"360\" w:footer=\"360\" w:gutter=\"0\"/>"
        "</w:sectPr>"
    )
    return (
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>"
        "<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">"
        f"<w:body>{body}{section_props}</w:body>"
        "</w:document>"
    )


def write_docx(lines: list[str]) -> None:
    content_types = (
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>"
        "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">"
        "<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>"
        "<Default Extension=\"xml\" ContentType=\"application/xml\"/>"
        "<Override PartName=\"/word/document.xml\" "
        "ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/>"
        "</Types>"
    )
    root_rels = (
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>"
        "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">"
        "<Relationship Id=\"rId1\" "
        "Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" "
        "Target=\"word/document.xml\"/>"
        "</Relationships>"
    )
    with ZipFile(CODECAT_DOCX, "w", compression=ZIP_DEFLATED) as docx:
        docx.writestr("[Content_Types].xml", content_types)
        docx.writestr("_rels/.rels", root_rels)
        docx.writestr("word/document.xml", build_document_xml(lines))


def main() -> None:
    src_dirs = find_src_dirs()
    if not src_dirs:
        raise SystemExit("No src/ directories found under any subdirectory of project root")

    file_entries = rust_source_files(src_dirs)
    if not file_entries:
        src_list = ", ".join(str(d.relative_to(ROOT)) for d in src_dirs)
        raise SystemExit(f"No .rs files found under: {src_list}")

    total_lines = write_numbered_codecat(non_empty_source_lines(file_entries))
    numbered_lines = CODECAT_RS.read_text(encoding="utf-8").splitlines()
    selected_lines = docx_lines(numbered_lines)
    write_docx(selected_lines)

    print(f"Source directories: {len(src_dirs)}")
    for d in src_dirs:
        print(f"  {d.relative_to(ROOT)}")
    print(f"Rust files: {len(file_entries)}")
    print(f"First file: {file_entries[0][1].relative_to(ROOT)}")
    print(f"codecat.rs lines: {total_lines}")
    print(f"codecat.docx lines: {len(selected_lines)}")
    print(f"Lines per page: {DOCX_LINES_PER_PAGE}")


if __name__ == "__main__":
    main()