# Naygo — valida que los workflows fijen acciones externas a commits inmutables.
# Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
# SPDX-License-Identifier: MIT

from __future__ import annotations

import re
import sys
from pathlib import Path

USES_LINE = re.compile(r"^\s*(?:-\s*)?uses:\s*(\S+)")
COMMIT_SHA = re.compile(r"^[0-9a-f]{40}$")
IMAGE_DIGEST = re.compile(r"^sha256:[0-9a-f]{64}$")


def collect_errors(workflows_dir: Path) -> list[str]:
    """Return workflow action references that are not pinned immutably."""
    files = sorted((*workflows_dir.glob("*.yml"), *workflows_dir.glob("*.yaml")))
    errors: list[str] = []

    if not files:
        return [f"No se encontraron workflows YAML en {workflows_dir}."]

    for workflow in files:
        for line_number, line in enumerate(workflow.read_text(encoding="utf-8").splitlines(), 1):
            match = USES_LINE.match(line)
            if not match:
                continue

            reference = match.group(1)
            if reference.startswith("./"):
                continue

            if reference.startswith("docker://"):
                image_reference = reference.removeprefix("docker://")
                if "@" not in image_reference or not IMAGE_DIGEST.fullmatch(
                    image_reference.rsplit("@", 1)[-1]
                ):
                    errors.append(
                        f"{workflow}:{line_number}: imagen Docker sin digest SHA-256: {reference}"
                    )
                continue

            if "@" not in reference:
                errors.append(
                    f"{workflow}:{line_number}: acción sin SHA completo: {reference}"
                )
                continue

            _, revision = reference.rsplit("@", 1)
            if not COMMIT_SHA.fullmatch(revision):
                errors.append(
                    f"{workflow}:{line_number}: revisión no inmutable (se requiere SHA de 40 "
                    f"caracteres): {reference}"
                )

    return errors


def main() -> int:
    repo_root = Path(__file__).resolve().parents[1]
    workflows_dir = repo_root / ".github" / "workflows"
    errors = collect_errors(workflows_dir)
    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1

    print(f"Pins de acciones válidos en {workflows_dir}.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
