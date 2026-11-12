#!/usr/bin/env python3
"""Freeze approved internal scope and actual native API declarations, not validator readiness."""

import argparse
import hashlib
import json
from pathlib import Path
import sys
import tomllib

from collect_compiler_inventory import validate_existing
from evidence import load_json


ROOT = Path(__file__).resolve().parents[1]
INPUTS = (
    "config/support-matrix.json", "config/toolchain.json", "validation/source-provenance.json",
    "Cargo.toml", "verification/Cargo.toml", "tools/check_internal_profile.py",
    ".github/workflows/ci.yml", "tools/check_workflows.py",
)
API_CATEGORIES = {
    "root_reexported_function", "public_inherent_method_on_root_type", "root_trait_method",
}
FROZEN_SCOPE_INPUTS = {
    "config/support-matrix.json": "cab4d567081ebac018a3f38849aa02456792ba3b94efcd5f3adfe512866380be",
    "validation/source-provenance.json": "95e21a8330540c2f1be6ae7446764a97047ee6cd9412516775da935f3c3b8571",
}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def scope_digest(path):
    return hashlib.sha256(path.read_bytes().replace(b"\r\n", b"\n")).hexdigest()


def snapshot(root, compiler_directory):
    for name in INPUTS:
        path = root / name
        if path.is_symlink() or not path.resolve().is_relative_to(root.resolve()):
            raise ValueError("internal profile input is symlinked or outside the checkout")
    matrix = load_json(root / "config/support-matrix.json")
    provenance = load_json(root / "validation/source-provenance.json")
    toolchain = load_json(root / "config/toolchain.json")
    package = tomllib.loads((root / "Cargo.toml").read_text())["package"]
    proof = tomllib.loads((root / "verification/Cargo.toml").read_text())["package"]
    if (
        matrix["approval"]["status"] != "approved_with_delegated_limit_defaults"
        or matrix["public_api"]["status"] != "approved"
        or matrix["normative_baseline"]["itu"] != "ITU-T T.88 (08/2018)"
        or matrix["normative_baseline"]["iso"] != "ISO/IEC 14492:2019, Edition 2"
        or not (
            package["version"] in {"0.2.0-dev", "0.3.0-dev"} and package["publish"] is False
            or package["version"] == "0.2.0" and package["publish"] == ["crates-io"]
        )
        or proof["rust-version"] != toolchain["verus"]["rust_toolchain"]
        or provenance["policy"]["third_party_uploads_permitted"] is not False
    ):
        raise ValueError("internal profile needs the approved release/development scope and toolchain")
    rows = matrix["rows"]
    if (
        not rows or len({row["feature"] for row in rows}) != len(rows)
        or any(row["status"] not in {"required", "deferred", "unsupported", "prohibited"}
               or not row["clauses"] for row in rows)
    ):
        raise ValueError("internal profile has duplicate, empty or unclassified feature scope")
    documents = {row["id"]: row for row in provenance["normative_documents"]}
    references = {row["id"]: row for row in provenance["normative_references"]}
    if (
        documents["ITU-T T.88 (08/2018)"]["edition"] != "2.0"
        or documents["ISO/IEC 14492:2019"]["edition"] != "2"
        or "ITU-T T.6 (1988)" not in references
        or len(documents) != len(provenance["normative_documents"])
        or len(references) != len(provenance["normative_references"])
    ):
        raise ValueError("internal scope must preserve distinct T.88/ISO/T.6 edition metadata")
    if any(scope_digest(root / name) != expected for name, expected in FROZEN_SCOPE_INPUTS.items()):
        raise ValueError("approved scope metadata changed; an explicit reviewed profile update is required")
    compiler = validate_existing(root, compiler_directory)
    entries = []
    for row in compiler["records"]:
        surface = row["declaration_surface"]
        if surface["category"] not in API_CATEGORIES:
            continue
        if "compiler_signature" not in surface or not surface["root_aliases"]:
            raise ValueError("exposed API declaration lacks compiler signature or root ownership")
        entries.append({
            "id": row["id"], "source_span": row["source_span"],
            "compiler_id": row["compiler_id"], "compiler_has_body": row["compiler_has_body"],
            "declaration_surface": surface,
        })
    if len({row["id"] for row in entries}) != len(entries):
        raise ValueError("internal profile aliases distinct exposed API declarations")
    expected = sum(compiler["api_declaration_surfaces"]["categories"].get(name, 0)
                   for name in API_CATEGORIES)
    if len(entries) != expected or not entries:
        raise ValueError("internal profile omits compiler-derived exposed API declarations")
    return {
        "schema_version": 1, "status": "approved_internal_declaration_profile_snapshot",
        "source_bindings": {name: digest(root / name) for name in INPUTS},
        "compiler_report": {
            "sha256": digest(compiler_directory / "report.json"),
            "source_bindings": compiler["source_bindings"], "artifacts": compiler["artifacts"],
            "counts": compiler["counts"], "profile": compiler["profile"],
        },
        "package": package, "proof_package": proof,
        "toolchain": toolchain,
        "approved_support_matrix": matrix,
        "standards_metadata_only": {
            "normative_documents": provenance["normative_documents"],
            "normative_references": provenance["normative_references"],
            "processing_policy": provenance["policy"],
        },
        "public_root_exports": compiler["public_root_exports"],
        "api_declaration_surfaces": compiler["api_declaration_surfaces"],
        "declared_api_entries": sorted(entries, key=lambda row: row["id"]),
        "api_entry_count": len(entries),
        "source_statement_inventory_qualified": False,
        "code_domains_and_contract_baseline_qualified": False,
        "ready_for_external_validation": False,
        "validator_specific_work": "separate; identity/version/input adaptation and execution not required for this snapshot",
        "claim_boundary": (
            "All approved feature rows, requirements, exclusions, resource policy and release "
            "claims are retained. Compiler-root free/inherent/trait declarations retain exact "
            "signatures; constructors/trait implementations/closures remain visible in the "
            "bound original compiler inventory. This is not complete constructible API or "
            "reachability/domain qualification, standards-text processing, normative approval, "
            "exhaustive statements or full readiness. Metadata hashes are not source-text review."
        ),
    }


def validate_snapshot(root, compiler_directory, path):
    report = load_json(path)
    if report != snapshot(root, compiler_directory):
        raise ValueError("stale, truncated or altered internal profile snapshot")
    return report


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--compiler-inventory", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--check-existing", action="store_true")
    arguments = parser.parse_args()
    try:
        if arguments.check_existing:
            report = validate_snapshot(ROOT, arguments.compiler_inventory, arguments.output)
        else:
            report = snapshot(ROOT, arguments.compiler_inventory)
            arguments.output.parent.mkdir(parents=True, exist_ok=True)
            arguments.output.write_text(json.dumps(report, indent=2) + "\n")
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f"internal profile snapshot failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps({"status": report["status"], "api_entry_count": report["api_entry_count"],
                      "ready_for_external_validation": False}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
