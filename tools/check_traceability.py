#!/usr/bin/env python3
"""Check provisional normative links, not the correctness of standard translation."""

import argparse
import base64
import copy
from datetime import datetime
import hashlib
import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path, PurePosixPath
from urllib.parse import parse_qs, quote, urlsplit

from jsonschema import ValidationError, validate as validate_json

from evidence import invalid_constant, load_json, read_evidence, unique_object
from rust_source import contract_sections, items, parse, production_code, source_functions
from readiness_assessment import assess
from rust_tests import discover_tests as discover_cargo_tests, test_inventory


ROOT = Path(__file__).resolve().parents[1]
PREFIX = "jbig2_rs_production::"
REVIEW_REPOSITORY = "microsoft/jbig2_rs"
CLASSIFICATION_SCHEMA = {
    "type": "object", "additionalProperties": False,
    "required": ["review", "registry_sha256", "project_source_sha256", "families"],
    "properties": {
        "review": {"const": "author-navigation-only-not-independent-normative-approval"},
        "registry_sha256": {"$ref": "#/$defs/sha256"},
        "project_source_sha256": {"$ref": "#/$defs/sha256"},
        "families": {
            "type": "array", "minItems": 1,
            "items": {
                "type": "object", "additionalProperties": False,
                "required": ["file", "members", "classification", "justification", "ambiguity"],
                "properties": {
                    "file": {"type": "string", "pattern": r"^(specs|proofs)/.+\.rs$"},
                    "members": {
                        "type": "array", "minItems": 1, "uniqueItems": True,
                        "items": {"type": "string", "pattern": r"^(spec|proof|macro)::[A-Za-z_][A-Za-z0-9_]*$"},
                    },
                    "classification": {
                        "enum": ["project-result-model", "project-policy", "representation-invariant",
                                 "proof-support", "candidate-standard-facing-model", "classification-pending"],
                    },
                    "justification": {"$ref": "#/$defs/nonempty"},
                    "ambiguity": {"type": "string"},
                    "caveats": {"type": "string"},
                },
                "allOf": [{
                    "if": {"properties": {"classification": {"const": "classification-pending"}}},
                    "then": {"properties": {"ambiguity": {"$ref": "#/$defs/nonempty"}}},
                }],
            },
        },
    },
}


def classification_evidence(root, ledger):
    """Conservative project-source binding, not a normative dependency closure."""
    paths = sorted(
        path.relative_to(root).as_posix()
        for directory in ("src", "specs", "proofs")
        for path in (root / directory).rglob("*.rs")
    )
    sources = {
        name: hashlib.sha256(source_path(root, name).read_bytes()).hexdigest()
        for name in paths
    }
    return {
        field: hashlib.sha256(json.dumps(
            value, sort_keys=True, separators=(",", ":"),
        ).encode("utf-8")).hexdigest()
        for field, value in (
            ("registry_sha256", ledger["definitions"]),
            ("project_source_sha256", sources),
        )
    }


def check_owner_review_confirmation(root, ledger):
    confirmation = ledger.get("owner_review_confirmation")
    if confirmation is None:
        return None
    try:
        confirmed_at = datetime.fromisoformat(confirmation["confirmed_at"])
    except (TypeError, ValueError):
        raise ValueError("owner review confirmation requires a timezone-qualified timestamp") from None
    if confirmed_at.tzinfo is None:
        raise ValueError("owner review confirmation requires a timezone-qualified timestamp")
    sources = classification_evidence(root, ledger)
    requirements_hash = hashlib.sha256(json.dumps(
        ledger["requirements"], sort_keys=True, separators=(",", ":"),
    ).encode("utf-8")).hexdigest()
    if (
        confirmation["project_source_sha256"] != sources["project_source_sha256"]
        or confirmation["requirements_sha256"] != requirements_hash
    ):
        raise ValueError("owner review confirmation does not match the current source/requirement snapshot")
    return copy.deepcopy(confirmation)


def audit_declarations(root, ledger):
    """Reuse masked production source and module resolution for exact membership."""
    registered = {}
    for identity, definition in ledger["definitions"].items():
        key = (definition["file"], definition["kind"], definition["name"])
        if key in registered:
            raise ValueError(f"duplicate audit declaration identity: {key}")
        check_definition(root, definition, None)
        registered[key] = identity
    declarations = set(registered)
    paths = {
        path.relative_to(root).as_posix()
        for directory in ("specs", "proofs/model_support")
        for path in (root / directory).rglob("*.rs")
    } | {key[0] for key in registered}
    for relative in sorted(paths):
        code = production_code(source_path(root, relative).read_bytes(), exclude_proofs=False)
        seen = set()
        for match in re.finditer(r"\b(spec|proof)\s+fn\s+([A-Za-z_]\w*)\b", code):
            key = (relative, match[1], match[2])
            if key in seen:
                raise ValueError(f"ambiguous audit declaration identity: {key}")
            seen.add(key)
            if match[1] == "spec" and relative.startswith(("specs/", "proofs/model_support/")):
                declarations.add(key)
    return declarations, registered


def audit_classifications(root, ledger, declarations):
    authored = ledger.get("model_classifications")
    # Keep unrelated source-bound receipts over the shared evidence schema intact.
    validate_json(authored, {
        "$defs": load_json(Path(__file__).with_name("evidence.schema.json"))["$defs"],
        **CLASSIFICATION_SCHEMA,
    })
    expected = classification_evidence(root, ledger)
    if any(authored[field] != value for field, value in expected.items()):
        raise ValueError("stale navigation classification source/binding evidence")
    classifications = {}
    for family in authored["families"]:
        if not family["justification"].strip() or (
            family["classification"] == "classification-pending" and not family["ambiguity"].strip()
        ):
            raise ValueError("navigation classification needs purpose evidence or unresolved ambiguity")
        for member in family["members"]:
            kind, name = member.split("::")
            key = (family["file"], kind, name)
            if key in classifications:
                raise ValueError(f"duplicate navigation classification member: {key}")
            classifications[key] = {
                "classification": family["classification"],
                "classification_review": authored["review"],
                "justification": family["justification"],
                "ambiguity": family["ambiguity"],
                "caveats": family.get("caveats", ""),
            }
    if set(classifications) != declarations:
        missing = sorted(declarations - set(classifications))
        unknown = sorted(set(classifications) - declarations)
        raise ValueError(f"navigation classification membership differs: missing={missing}, unknown={unknown}")
    return classifications


def navigation_declarations(source, selected=None):
    """Locate only unambiguous declarations using the existing masked source."""
    root = parse(source)
    code = production_code(source, exclude_proofs=False).encode("utf-8").decode("latin1")
    pattern = (
        r"(?m)^[ \t]*(?:pub(?:\([^)]*\))?\s+)?(?:(?:open|closed|opaque)\s+)?"
        r"(?:(spec|proof)\s+fn\s+([A-Za-z_]\w*)|macro_rules!\s*([A-Za-z_]\w*))\b"
    )
    declarations = {}
    for match in re.finditer(pattern, code):
        key = ("macro", match[3]) if match[3] else (match[1], match[2])
        if selected is not None and key not in selected:
            continue
        start = match.start() + len(match[0]) - len(match[0].lstrip())
        scope = root.descendant_for_byte_range(start, start + 1).parent
        if key[0] == "macro":
            owner = scope.parent if scope.type == "macro_definition" else None
        else:
            invocation = scope.parent
            name = invocation.child_by_field_name("macro") if invocation is not None else None
            owner = invocation.parent if (
                scope.type == "token_tree" and invocation.type == "macro_invocation"
                and name is not None and name.text == b"verus"
            ) else None
        if owner is not None and owner.type == "expression_statement":
            owner = owner.parent
        if owner is None or owner.type != "source_file":
            raise ValueError(f"unsupported inline navigation owner/placement: {key}")
        if key in declarations:
            raise ValueError(f"ambiguous inline navigation placement: {key}")
        declarations[key] = match.start()
    return declarations


def navigation_context(ledger, identity):
    """One existing row is a reading entry point, never a new normative citation."""
    return next((
        row for row in ledger["requirements"]
        if identity in row["models"] + row["proof_macros"]
    ), None) if identity else None


def audit_inline_navigation(root, ledger, declarations, registered, classifications):
    covered = set()
    inline_references = set()
    # Scan all project sources, not just specs: misplaced references must not hide
    # in unregistered support files or in the runtime source.
    paths = sorted(
        path.relative_to(root).as_posix()
        for directory in ("src", "specs", "proofs")
        for path in (root / directory).rglob("*.rs")
    )
    for relative in paths:
        source = source_path(root, relative).read_bytes()
        code = production_code(source, exclude_proofs=False).encode("utf-8").decode("latin1")
        active = production_code(
            source, exclude_proofs=False, mask_noncode=False,
        ).encode("utf-8").decode("latin1")
        positions = navigation_declarations(source, {
            (kind, name) for file, kind, name in declarations if file == relative
        })
        comments = []

        def visit(node):
            if node.type == "line_comment":
                start, end = node.start_byte, node.end_byte
                if active[start:end].strip() and not code[start:end].strip():
                    comments.append((start, end, node.text.decode("utf-8").removesuffix("\r")))
            for child in node.named_children:
                visit(child)

        visit(parse(source))
        for _, end, text in comments:
            if not re.match(r"//\s*(?:Audit|Model navigation):", text):
                continue
            # Only whitespace/comments may separate a marker from its declaration.
            following = next((index for index in range(end, len(code)) if not code[index].isspace()), len(code))
            targets = [
                (kind, name) for (kind, name), offset in positions.items()
                if offset <= following
                and not code[offset:following].strip()
            ]
            if len(targets) != 1:
                raise ValueError(f"ambiguous or misplaced inline navigation: {relative}:{text}")
            kind, name = targets[0]
            key = (relative, kind, name)
            if key not in declarations:
                raise ValueError(f"unknown inline navigation declaration: {key}")
            if text.startswith("// Audit:"):
                identity = text.removeprefix("// Audit:").strip()
                if registered.get(key) != identity or identity in inline_references:
                    raise ValueError(f"unknown, duplicate or misrouted inline audit reference: {identity}")
                inline_references.add(identity)
                continue
            expected = f"// Model navigation: {kind}::{name} [{classifications[key]['classification']}]"
            if text != expected or key in covered:
                raise ValueError(f"unknown, duplicate or misplaced inline model reference: {relative}:{text}")
            # Keep purpose prose authored, but require a visible local explanation
            # and an exact inherited entry context (or an explicit mapping gap).
            block = source[end:positions[(kind, name)]].decode("utf-8")
            if not re.search(r"(?m)^// Purpose: \S", block):
                raise ValueError(f"missing inline navigation purpose: {key}")
            context = navigation_context(ledger, registered.get(key))
            expected_context = (
                f"// REQUIREMENT CONTEXT: {context['id']} | {context['standard']} | {context['clause']}"
                if context else "// Mapping: no registered requirement/locator mapping."
            )
            contexts = [
                line for line in block.splitlines()
                if line.startswith(("// REQUIREMENT CONTEXT:", "// Mapping:"))
            ]
            if contexts != [expected_context]:
                raise ValueError(f"missing or incorrect inline requirement context: {key}")
            if "// Fidelity: unresolved; no independent normative approval." not in block.splitlines():
                raise ValueError(f"missing inline fidelity boundary: {key}")
            covered.add(key)
    missing = declarations - covered
    missing_references = set(registered.values()) - inline_references
    if missing or missing_references:
        raise ValueError(
            f"missing inline navigation: declarations={sorted(missing)}, references={sorted(missing_references)}"
        )
    return covered, inline_references


def audit_inventory(root, ledger):
    declarations, registered = audit_declarations(root, ledger)
    classifications = audit_classifications(root, ledger, declarations)
    covered, inline_references = audit_inline_navigation(root, ledger, declarations, registered, classifications)
    references = {
        identity: [
            {"id": row["id"], "standard": row["standard"], "clause": row["clause"],
             "review": row["review"], "obligation_kind": row.get("obligation_kind", "normative_rule")}
            for row in ledger["requirements"]
            if identity in row["models"] + row["proof_macros"]
        ]
        for identity in ledger["definitions"]
    }
    definitions = []
    for identity, definition in ledger["definitions"].items():
        definitions.append({
            "id": identity, **copy.deepcopy(definition),
            **classifications[(definition["file"], definition["kind"], definition["name"])],
            "requirements": references[identity],
            "normative_approval": False,
        })
    unregistered = [
        {"file": file, "kind": kind, "name": name, **classifications[(file, kind, name)],
         "requirements": [], "normative_approval": False}
        for file, kind, name in sorted(declarations - set(registered))
    ]
    implementation_dependencies = {}
    for directory in ("specs", "proofs/model_support"):
        for path in sorted((root / directory).rglob("*.rs")):
            relative = path.relative_to(root).as_posix()
            source = source_path(root, relative).read_bytes()
            code = production_code(source, exclude_proofs=False)
            names = sorted(set(re.findall(r"\bsuper::([A-Z][A-Z_0-9]+)\b", code)))
            if names:
                implementation_dependencies[relative] = names
    for definition in definitions:
        definition["inline_audit_reference"] = definition["id"] in inline_references
    return {
        "scope": "Registered ledger definitions and all spec functions in specs/ and proofs/model_support/. Other unregistered proof/view declarations are outside this bounded inventory. Purpose only, not proof strength, normative decomposition or transitive dependency analysis.",
        "registered_definition_count": len(definitions),
        "registered_spec_count": sum(row["kind"] == "spec" for row in definitions),
        "inline_navigation": {
            "declared_count": len(declarations),
            "covered_count": len(covered),
            "uncovered_count": len(declarations - covered),
            "registered_reference_count": len(inline_references),
            "structural_coverage_complete": covered == declarations,
            "independently_verified_citations": False,
            "normative_approval": False,
            "document_processing_authorized": False,
        },
        "definitions": definitions,
        "unregistered_spec_functions": unregistered,
        "implementation_constant_references": implementation_dependencies,
        "implementation_dependency_review": "Lexical implementation constants/tables are dependencies to review, not normative ground truth; source binding conservatively covers all project Rust.",
        "classification_counts": {
            purpose: sum(row["classification"] == purpose for row in classifications.values())
            for purpose in sorted({row["classification"] for row in classifications.values()})
        },
        "unresolved_declarations": [
            {"file": file, "kind": kind, "name": name, "ambiguity": row["ambiguity"]}
            for (file, kind, name), row in sorted(classifications.items())
            if row["classification"] == "classification-pending" or row["ambiguity"].strip()
        ],
        "classification_complete": all(
            row["classification"] != "classification-pending" and not row["ambiguity"].strip()
            for row in classifications.values()
        ),
        "normative_approval": False,
    }


def audit_packet(root, ledger, requirement_ids, ledger_path):
    if (
        not isinstance(requirement_ids, list) or not requirement_ids
        or not all(isinstance(value, str) for value in requirement_ids)
        or len(set(requirement_ids)) != len(requirement_ids)
    ):
        raise ValueError("audit packets need nonempty unique requirement IDs")
    if load_json(source_path(root, ledger_path)) != ledger:
        raise ValueError("audit ledger changed before snapshot creation")
    selected = {row["id"]: row for row in ledger["requirements"]}
    if any(identity not in selected for identity in requirement_ids):
        raise ValueError("unknown audit requirement ID")
    requirements = [selected[identity] for identity in requirement_ids]
    definitions = {
        identity: ledger["definitions"][identity]
        for row in requirements for identity in row["models"] + row["proof_macros"]
    }
    for directory in ("src", "specs", "proofs", "tools", ".github/agents", "verification/stdlib"):
        if any(path.is_symlink() for path in (root / directory).rglob("*")):
            raise ValueError("audit source snapshots must not contain symlinks")
    paths = {
        path.relative_to(root).as_posix()
        for directory in ("src", "specs", "proofs")
        for path in (root / directory).rglob("*.rs")
    }
    paths.update(
        path.relative_to(root).as_posix()
        for pattern in ("*.py", "*.json", "*.txt")
        for path in (root / "tools").rglob(pattern)
        if "__pycache__" not in path.relative_to(root).parts
    )
    paths.update(
        path.relative_to(root).as_posix()
        for pattern in ("*.rs", "*.toml", "Cargo.lock", "*.json", "*.py", "*.txt")
        for path in (root / "verification/stdlib").rglob(pattern)
        if not any(part in {"target", "__pycache__", "corpus", "artifacts"}
                   for part in path.relative_to(root).parts)
    )
    paths.update((
        ledger_path, "validation/source-provenance.json",
        "config/support-matrix.json", "config/toolchain.json",
        "tools/check_traceability.py", "tools/evidence.schema.json",
        "Cargo.toml", "Cargo.lock", "verification/Cargo.toml", "verification/Cargo.lock",
    ))
    paths.update(
        path.relative_to(root).as_posix()
        for path in (root / ".github/agents").glob("*.agent.md")
    )
    for name in ("specs/README.md", "docs/specification-review.md"):
        if (root / name).is_file():
            paths.add(name)
    sources = {}
    for relative in sorted(paths):
        unresolved = root / relative
        if unresolved.is_symlink() or any(parent.is_symlink() for parent in unresolved.parents if parent != root):
            raise ValueError("audit source snapshots must not contain symlinks")
        sources[relative] = hashlib.sha256(source_path(root, relative).read_bytes()).hexdigest()
    packet = {
        "schema_version": 1,
        "state": "unreviewed",
        "scope": "Conservative runtime/model/proof and local standard-interface source review snapshot, not an approved normative-only model freeze.",
        "requirement_ids": list(requirement_ids),
        "standards": [
            copy.deepcopy(row) for row in ledger["standards"]
            if row["id"] in {requirement["standard"] for requirement in requirements}
        ],
        "requirements": copy.deepcopy(requirements),
        "definitions": copy.deepcopy(definitions),
        "source_bindings": sources,
        "human_adjudicator": None,
        "normative_approval": False,
        "document_processing_authorized": False,
    }
    packet["packet_sha256"] = hashlib.sha256(json.dumps(
        packet, sort_keys=True, separators=(",", ":"), allow_nan=False,
    ).encode("utf-8")).hexdigest()
    if any(
        hashlib.sha256(source_path(root, name).read_bytes()).hexdigest() != value
        for name, value in sources.items()
    ):
        raise ValueError("audit source changed during snapshot creation")
    return packet


def check_audit_packet(root, ledger, packet, expected_sha256, ledger_path):
    if not isinstance(expected_sha256, str) or not re.fullmatch(r"[0-9a-f]{64}", expected_sha256):
        raise ValueError("an independently supplied SHA-256 pin is required")
    if not isinstance(packet, dict):
        raise ValueError("audit packet must be a mapping")
    payload = dict(packet)
    declared = payload.pop("packet_sha256", None)
    actual = hashlib.sha256(json.dumps(
        payload, sort_keys=True, separators=(",", ":"), allow_nan=False,
    ).encode("utf-8")).hexdigest()
    if declared != expected_sha256 or actual != expected_sha256:
        raise ValueError("modified or unpinned audit packet")
    expected = audit_packet(root, ledger, packet.get("requirement_ids"), ledger_path)
    if packet != expected or expected["packet_sha256"] != expected_sha256:
        raise ValueError("stale, modified or unpinned audit packet; no normative approval is inferred")


def github_review_json(endpoint):
    return json.loads(subprocess.check_output([
        "gh", "api", "-H", "X-GitHub-Api-Version: 2022-11-28",
        f"repos/{REVIEW_REPOSITORY}" + (f"/{endpoint}" if endpoint else ""),
    ], encoding="utf-8", stdin=subprocess.DEVNULL, timeout=60),
        object_pairs_hook=unique_object, parse_constant=invalid_constant)


def source_review_disposition(pr_number, review_id, revision, packet_pin, fetch=github_review_json):
    """Authenticate an explicit source-snapshot disposition, never normative approval."""
    if (
        type(pr_number) is not int or pr_number <= 0
        or type(review_id) is not int or review_id <= 0
        or not isinstance(revision, str) or not re.fullmatch(r"[0-9a-f]{40}", revision)
        or not isinstance(packet_pin, str) or not re.fullmatch(r"[0-9a-f]{64}", packet_pin)
    ):
        raise ValueError("source review needs exact positive IDs and pinned source identities")
    repository = fetch("")
    branch = repository["default_branch"]
    if repository["full_name"] != REVIEW_REPOSITORY or not isinstance(branch, str) or not branch:
        raise ValueError("source review needs the repository's configured default branch")
    default_ref = fetch(f"git/ref/heads/{quote(branch, safe='/')}")
    authority_revision = default_ref["object"]["sha"]
    pull = fetch(f"pulls/{pr_number}")
    base = pull["base"]
    if (
        pull["number"] != pr_number or pull["state"] != "open"
        or pull["head"]["sha"] != revision
        or base["ref"] != branch or base["sha"] != authority_revision
        or base["repo"]["full_name"] != REVIEW_REPOSITORY
        or pull["head"]["repo"]["full_name"] != REVIEW_REPOSITORY
        or default_ref["ref"] != f"refs/heads/{branch}"
        or default_ref["object"]["type"] != "commit"
        or not re.fullmatch(r"[0-9a-f]{40}", base["sha"])
    ):
        raise ValueError("source review PR/repository/head identity differs")
    provenance = fetch(f"contents/validation/source-provenance.json?ref={base['sha']}")
    if provenance.get("type") != "file" or provenance.get("encoding") != "base64":
        raise ValueError("source review authority must be a base-revision provenance file")
    recorded = json.loads(
        base64.b64decode(
            provenance["content"].replace("\n", "").replace("\r", ""), validate=True,
        ).decode("utf-8"),
        object_pairs_hook=unique_object, parse_constant=invalid_constant,
    )
    authority = recorded.get("policy", {}).get("source_review_authority")
    if (
        not isinstance(authority, dict)
        or set(authority) != {"enabled", "human_user_ids"}
        or authority["enabled"] is not True
        or not isinstance(authority["human_user_ids"], list)
        or not authority["human_user_ids"]
        or any(type(value) is not int or value <= 0 for value in authority["human_user_ids"])
        or len(set(authority["human_user_ids"])) != len(authority["human_user_ids"])
    ):
        raise ValueError("base-revision human source-review authority is not configured")
    reviews = []
    for page in range(1, 22):
        batch = fetch(f"pulls/{pr_number}/reviews?per_page=100&page={page}")
        if not isinstance(batch, list) or len(batch) > 100:
            raise ValueError("invalid source review pagination")
        if page == 21:
            if batch:
                raise ValueError("source review history exceeds the supported bound")
            break
        reviews.extend(batch)
        if len(batch) < 100:
            break
    if any(type(row.get("id")) is not int or row["id"] <= 0 for row in reviews):
        raise ValueError("invalid authenticated source review identity")
    if len({row["id"] for row in reviews}) != len(reviews):
        raise ValueError("duplicate authenticated source review identity")
    selected = [row for row in reviews if row["id"] == review_id]
    if len(selected) != 1:
        raise ValueError("explicit human source review is missing")
    review = selected[0]
    user = review["user"]
    if (
        review["state"] != "APPROVED" or review["commit_id"] != revision
        or user.get("type") != "User"
        or type(user.get("id")) is not int
        or user["id"] not in authority["human_user_ids"]
        or user["id"] == pull["user"]["id"]
        or not isinstance(review.get("body"), str)
        or review["body"].strip() != f"JBIG2-SOURCE-REVIEW-SHA256: {packet_pin}"
    ):
        raise ValueError("human source review is stale, unauthorized, self-authored or unpinned")
    decisive = [
        row for row in reviews if row["user"]["id"] == user["id"]
        and row["state"] in {"APPROVED", "CHANGES_REQUESTED", "DISMISSED"}
    ]
    for row in decisive:
        if not isinstance(row.get("submitted_at"), str) or not re.fullmatch(
            r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", row["submitted_at"],
        ):
            raise ValueError("decisive source reviews need exact submission timestamps")
        datetime.strptime(row["submitted_at"], "%Y-%m-%dT%H:%M:%SZ")
    if any(
        row["id"] != review_id and row["submitted_at"] >= review["submitted_at"]
        for row in decisive
    ):
        raise ValueError("human source review was superseded")
    return {
        "status": "authenticated_source_snapshot_disposition",
        "repository": REVIEW_REPOSITORY, "pull_request": pr_number,
        "review_id": review_id, "human_user_id": user["id"],
        "source_revision": revision, "packet_sha256": packet_pin,
        "authority_revision": base["sha"],
        "scope": "Source-snapshot review only; not a normative-only model freeze or semantic certificate.",
        "normative_approval": False, "document_processing_authorized": False,
    }


def source_path(root, relative):
    parts = PurePosixPath(relative)
    if parts.is_absolute() or ".." in parts.parts or "\\" in relative:
        raise ValueError(f"invalid traceability source path: {relative}")
    path = (root / relative).resolve()
    if not path.is_relative_to(root.resolve()) or not path.is_file():
        raise ValueError(f"missing or external source: {relative}")
    return path


def resolve_module(root, names):
    path = root / "src/lib.rs"
    visited = {path.relative_to(root).as_posix()}
    for name in names:
        candidates = [
            (node, attributes) for node, attributes in items(parse(path.read_bytes()), False)
            if node.type == "mod_item"
            and node.child_by_field_name("name").text.decode() == name
        ]
        if len(candidates) != 1:
            raise ValueError(f"missing or ambiguous module: {'::'.join(names)}")
        node, attributes = candidates[0]
        if node.child_by_field_name("body") is not None:
            raise ValueError("inline model module bindings need explicit support")
        paths = [
            re.fullmatch(r'#\[\s*path\s*=\s*"([^"]+)"\s*\]', attribute.text.decode())
            for attribute in attributes
        ]
        paths = [match[1] for match in paths if match]
        directory = path.parent
        if paths:
            choices = [directory / paths[0]]
        else:
            if path.name not in {"lib.rs", "mod.rs"}:
                directory /= path.stem
            choices = [directory / f"{name}.rs", directory / name / "mod.rs"]
        existing = [choice.resolve() for choice in choices if choice.is_file()]
        if len(existing) != 1 or not existing[0].is_relative_to(root):
            raise ValueError(f"missing, ambiguous or external module file: {name}")
        path = existing[0]
        visited.add(path.relative_to(root).as_posix())
    return path, visited


def module_sources(root, path):
    sources = set()

    def visit(current):
        if current in sources:
            raise ValueError(f"cyclic or duplicate source inclusion: {current}")
        sources.add(current)
        source = current.read_bytes()
        includes = []
        for node, _ in items(parse(source), False):
            if node.type == "expression_statement" and len(node.named_children) == 1:
                node = node.named_children[0]
            if node.type != "macro_invocation":
                continue
            name = node.child_by_field_name("macro")
            if name is not None and name.text == b"include":
                includes.append(node)
        code = production_code(source, exclude_proofs=False)
        if len(re.findall(r"\binclude\s*!\s*[\(\[{]", code)) != len(includes):
            raise ValueError("only module-scope source inclusion is supported")
        for node in includes:
            literal = re.fullmatch(
                r'include\s*!\s*\(\s*"([^"\\\n]+)"\s*\)', node.text.decode(),
            )
            if literal is None or (
                PurePosixPath(literal[1]).is_absolute() or Path(literal[1]).is_absolute()
            ):
                raise ValueError("source inclusion requires a literal relative path")
            included = (current.parent / literal[1]).resolve()
            if not included.is_relative_to(root.resolve()) or not included.is_file():
                raise ValueError("missing or external included source")
            if any(node.type == "mod_item" for node, _ in items(parse(included.read_bytes()), False)):
                raise ValueError("included module declarations need explicit support")
            visit(included)

    visit(path.resolve())
    return sources


def check_definition(root, definition, verifier_names):
    path, used = resolve_module(root, definition["module_path"])
    sources = module_sources(root, path)
    declared = source_path(root, definition["file"])
    if declared not in sources:
        raise ValueError(f"definition module points to another file: {definition['name']}")
    used.update(source.relative_to(root).as_posix() for source in sources)
    if definition["kind"] == "macro":
        declarations = [
            source for source in sources
            for node in parse(source.read_bytes()).named_children
            if node.type == "macro_definition"
            and node.child_by_field_name("name").text.decode() == definition["name"]
        ]
        if declarations != [declared]:
            raise ValueError(f"missing or ambiguous proof macro: {definition['name']}")
    else:
        pattern = (
            r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?(?:(?:open|closed|opaque)\s+)?"
            + definition["kind"] + r"\s+fn\s+" + re.escape(definition["name"]) + r"\b"
        )
        declarations = [
            source for source in sources
            for _ in re.finditer(
                pattern, production_code(source.read_bytes(), exclude_proofs=False),
            )
        ]
        if declarations != [declared]:
            raise ValueError(f"missing or ambiguous Verus declaration: {definition['name']}")
        qualified = PREFIX + "::".join([*definition["module_path"], definition["name"]])
        if verifier_names is not None and qualified not in verifier_names:
            raise ValueError(f"definition absent from verifier report: {qualified}")
    return used


def runtime_inventory(root):
    records = {}
    counts = {}
    for record in source_functions(root):
        base = record["id"]
        counts[base] = counts.get(base, 0) + 1
        identity = base if counts[base] == 1 else f"{base}#{counts[base]}"
        records[identity] = record
    return records


def compact(text):
    return re.sub(r"\s+", "", text)


def policy_row_hash(row):
    return hashlib.sha256(json.dumps(
        row, sort_keys=True, separators=(",", ":"),
    ).encode("utf-8")).hexdigest()


def resolve_standards(ledger, provenance):
    if len({entry["id"] for entry in ledger["standards"]}) != len(ledger["standards"]):
        raise ValueError("duplicate normative standard identity")
    documents = {}
    for standard in ledger["standards"]:
        candidates = [
            (collection, entry)
            for collection in ("normative_documents", "normative_references")
            for entry in provenance.get(collection, [])
            if entry["id"] == standard["id"]
        ]
        if len(candidates) != 1:
            raise ValueError(f"missing or ambiguous normative provenance: {standard['id']}")
        collection, document = candidates[0]
        if collection != standard.get("provenance_collection", "normative_documents"):
            raise ValueError(f"normative provenance collection differs: {standard['id']}")
        if not document.get("document_sha256"):
            raise ValueError(f"normative source has no approved document identity: {standard['id']}")
        if document["document_sha256"] != standard["document_sha256"]:
            raise ValueError("normative document identity differs from provenance")
        if document.get("edition") != standard["edition"]:
            raise ValueError("normative edition differs from provenance")
        documents[standard["id"]] = document
    return documents


def local_document_checks(ledger, standard_pdf=None, standard_documents=()):
    hashes = {standard["id"]: standard["document_sha256"] for standard in ledger["standards"]}
    checks = dict.fromkeys(hashes, False)
    selections = []
    if standard_pdf is not None:
        selections.append((ledger["standards"][0]["id"], Path(standard_pdf)))
    for selection in standard_documents:
        identity, separator, path = selection.partition("=")
        if not separator or not identity or not path:
            raise ValueError("--standard-document needs STANDARD_ID=PATH")
        selections.append((identity, Path(path)))
    seen = set()
    for identity, path in selections:
        if identity not in hashes:
            raise ValueError(f"unknown local normative document identity: {identity}")
        if identity in seen:
            raise ValueError(f"duplicate local normative document selection: {identity}")
        seen.add(identity)
        if hashlib.sha256(path.read_bytes()).hexdigest() != hashes[identity]:
            raise ValueError(f"local normative document hash mismatch: {identity}")
        checks[identity] = True
    return checks


def check_contract(record, section, tokens):
    if not tokens or any(not compact(token) for token in tokens):
        raise ValueError(f"empty contract evidence on {record['id']}")
    if any(compact(token).strip("()") == "true" for token in tokens):
        raise ValueError(f"tautological contract evidence on {record['id']}")
    if len({compact(token) for token in tokens}) != len(tokens):
        raise ValueError(f"duplicate contract token on {record['id']}")
    sections = [compact(text) for text in contract_sections(record, section, include_legacy=True)]
    if not sections or not all(any(compact(token) in text for text in sections) for token in tokens):
        raise ValueError(f"claimed {section} evidence is not present on {record['id']}")


def check(ledger, root, verifier_names=None, discovered=None):
    root = root.resolve()
    matrix = load_json(root / "config/support-matrix.json")
    provenance = load_json(root / "validation/source-provenance.json")
    documents = resolve_standards(ledger, provenance)
    consultations = ledger["source_consultations"]
    consulted = {entry["standard"] for entry in consultations}
    if len(consulted) != len(consultations) or consulted != {
        entry["id"] for entry in ledger["standards"]
    }:
        raise ValueError("source consultations must cover the exact normative standards once")
    for consultation in consultations:
        document = documents[consultation["standard"]]
        families = consultation["clause_families"]
        if (
            not families or any(not compact(family) for family in families)
            or len({compact(family) for family in families}) != len(families)
        ):
            raise ValueError("source consultation needs nonempty distinct clause families")
        if consultation["document_sha256"] != document["document_sha256"]:
            raise ValueError("source consultation document_sha256 differs from provenance")
        url = urlsplit(consultation["acquisition_url"])
        if (
            url.scheme != "https" or url.hostname != "www.itu.int"
            or url.hostname != urlsplit(document["source_url"]).hostname
            or url.path != "/rec/dologin_pub.asp"
            or url.username is not None or url.password is not None or url.fragment
        ):
            raise ValueError("source consultation is not the recorded official publication host")
        if consultation["acquisition_kind"] == "official_package":
            if not document.get("package_sha256") or consultation.get("package_sha256") != document["package_sha256"]:
                raise ValueError("source consultation package_sha256 differs from provenance")
            publication = urlsplit(document["source_url"]).path.rstrip("/").split("/")[-2]
            if parse_qs(url.query) != {
                "lang": ["e"], "id": [publication + "!!SOFT-ZST-E"], "type": ["items"],
            }:
                raise ValueError("source consultation URL identifies a different publication/package")
        elif consultation["acquisition_kind"] == "official_pdf":
            if "package_sha256" in consultation:
                raise ValueError("direct PDF consultation cannot invent a package identity")
            if consultation["acquisition_url"] != document["source_url"]:
                raise ValueError("direct PDF consultation URL differs from recorded provenance")
        else:
            raise ValueError("unsupported source consultation acquisition kind")
    features = {entry["feature"]: entry for entry in matrix["rows"]}
    if len(features) != len(matrix["rows"]):
        raise ValueError("duplicate support-matrix feature")
    inventory = {entry["feature"]: entry for entry in ledger["features"]}
    if len(inventory) != len(ledger["features"]) or set(features) != set(inventory):
        raise ValueError("feature worklist must cover the exact support matrix")
    for name, feature in features.items():
        if inventory[name]["support_status"] != feature["status"]:
            raise ValueError(f"stale feature scope: {name}")
        if inventory[name]["decomposition"] == "excluded" and feature["status"] != "deferred":
            raise ValueError(f"required/rejection/prohibited behavior cannot be excluded: {name}")
    coverage = read_evidence(root / "verification/production-proof-coverage.json", "coverage")
    coverage = {entry["id"]: entry for entry in coverage["functions"]}
    runtime = runtime_inventory(root)
    tests = test_inventory(root)
    used = set()
    referenced_definitions = {
        name for requirement in ledger["requirements"]
        for name in requirement["models"] + requirement["proof_macros"]
    }
    if referenced_definitions != set(ledger["definitions"]):
        raise ValueError("unused or missing model/proof definitions")
    definition_identities = [
        (entry["kind"], entry["file"], tuple(entry["module_path"]), entry["name"])
        for entry in ledger["definitions"].values()
    ]
    if len(definition_identities) != len(set(definition_identities)):
        raise ValueError("duplicate model/proof definition identity")
    for definition in ledger["definitions"].values():
        used.update(check_definition(root, definition, verifier_names))
    rows = []
    identifiers = set()
    for requirement in ledger["requirements"]:
        identity = requirement["id"]
        if identity in identifiers:
            raise ValueError(f"duplicate requirement ID: {identity}")
        identifiers.add(identity)
        if requirement["feature"] not in inventory:
            raise ValueError(f"unknown feature: {identity}")
        obligation_kind = requirement.get("obligation_kind", "normative_rule")
        policy = features[requirement["feature"]]
        if obligation_kind == "project_profile_policy":
            policy_source = requirement.get("policy_source", "feature_row")
            if policy_source == "feature_row":
                if not (policy.get("note") or policy.get("failure")):
                    raise ValueError(f"project policy has no explicit support-matrix basis: {identity}")
            elif policy_source == "public_api":
                policy = matrix.get("public_api", {})
                if policy.get("status") != "approved" or (
                    "Typed malformed, unsupported, resource-limit, input/output, and allocation errors."
                    not in policy.get("requirements", [])
                ):
                    raise ValueError(f"project error policy has no approved public API basis: {identity}")
            else:
                raise ValueError(f"unknown project policy source: {identity}")
            if requirement.get("policy_row_sha256") != policy_row_hash(policy):
                raise ValueError(f"stale or missing project policy evidence: {identity}")
        elif obligation_kind != "normative_rule" or "policy_row_sha256" in requirement or "policy_source" in requirement:
            raise ValueError(f"normative evidence cannot hide a project policy binding: {identity}")
        for field in ("runtime", "models", "proof_macros", "tests", "functional_evidence", "invariant_evidence"):
            values = [json.dumps(value, sort_keys=True) for value in requirement[field]]
            if len(values) != len(set(values)):
                raise ValueError(f"duplicate {field} evidence: {identity}")
        if requirement["standard"] not in {entry["id"] for entry in ledger["standards"]}:
            raise ValueError(f"unknown normative standard: {identity}")
        if identity.startswith("T6-") != (requirement["standard"] == "ITU-T T.6 (1988)"):
            raise ValueError(f"requirement prefix differs from normative source: {identity}")
        for definition in requirement["models"]:
            if ledger["definitions"][definition]["kind"] != "spec":
                raise ValueError(f"model reference is not a spec definition: {identity}")
        for definition in requirement["proof_macros"]:
            if ledger["definitions"][definition]["kind"] != "macro":
                raise ValueError(f"proof reference is not a macro definition: {identity}")
            macro = ledger["definitions"][definition]["name"]
            if not any(re.search(
                rf"\b{re.escape(macro)}\s*!",
                production_code(runtime[name]["body_text"].encode(), exclude_proofs=False),
            ) for name in requirement["runtime"]):
                raise ValueError(f"proof macro is not wired to recorded runtime: {identity}")
        for function in requirement["runtime"]:
            record = runtime[function]
            evidence = coverage[function]
            if record["file"] != evidence["file"] or record["file_sha256"] != evidence["file_sha256"]:
                raise ValueError(f"stale cached production evidence: {function}")
            used.add(record["file"])
            if verifier_names is not None and evidence["status"] == "verified":
                if not evidence["verus_functions"] or any(
                    name not in verifier_names for name in evidence["verus_functions"]
                ):
                    raise ValueError(f"runtime missing verifier-report evidence: {function}")
            if requirement["strength"] in {"functional_contract", "state_preservation", "safety_only"}:
                if evidence["status"] != "verified" or record["runtime_proof_divergence"]:
                    raise ValueError(f"formal claim has an unverified or divergent body: {identity}")
            if requirement["strength"] == "runtime_divergence" and evidence["status"] != "runtime-proof-divergence":
                raise ValueError(f"divergence classification changed: {identity}")
        if requirement["strength"] == "functional_contract" and not requirement["functional_evidence"]:
            raise ValueError(f"functional claim needs explicit postcondition evidence: {identity}")
        if requirement["strength"] == "state_preservation" and not requirement["invariant_evidence"]:
            raise ValueError(f"state claim needs explicit invariant evidence: {identity}")
        if requirement["strength"] != "functional_contract" and requirement["functional_evidence"]:
            raise ValueError(f"functional evidence has a weaker strength classification: {identity}")
        if any(claim["section"] != "ensures" for claim in requirement["functional_evidence"]):
            raise ValueError(f"preconditions cannot satisfy a functional result claim: {identity}")
        if any(claim["section"] != "ensures" for claim in requirement["invariant_evidence"]):
            raise ValueError(f"preconditions cannot satisfy a state preservation claim: {identity}")
        for claim in requirement["functional_evidence"] + requirement["invariant_evidence"]:
            if claim["runtime"] not in requirement["runtime"]:
                raise ValueError(f"contract points outside requirement runtime scope: {identity}")
            check_contract(runtime[claim["runtime"]], claim["section"], claim["tokens"])
        for reference in requirement["tests"]:
            key = (reference["target"], reference["name"])
            test = tests[key]
            if test["ignored"]:
                raise ValueError(f"ignored test cannot back a requirement: {key}")
            used.add(test["file"])
            if discovered is not None and key not in discovered:
                raise ValueError(f"test not present in Cargo discovery: {key}")
        rows.append({
            "id": identity, "clause": requirement["clause"], "title": requirement["title"],
            "standard": requirement["standard"],
            "feature": requirement["feature"],
            "obligation_kind": obligation_kind,
            "policy_row_sha256": requirement.get("policy_row_sha256"),
            "policy_source": requirement.get("policy_source", "feature_row") if obligation_kind == "project_profile_policy" else None,
            "strength": requirement["strength"], "review": requirement["review"],
            "runtime": requirement["runtime"], "gaps": requirement["gaps"],
            "scope_of_formal_result": requirement["scope_of_formal_result"],
            "assumptions": requirement["assumptions"],
            "runtime_locations": [
                {"name": name, "file": runtime[name]["file"], "line": runtime[name]["line"]}
                for name in requirement["runtime"]
            ],
            "models": [ledger["definitions"][name] for name in requirement["models"]],
            "proof_macros": [ledger["definitions"][name] for name in requirement["proof_macros"]],
            "tests": [tests[(test["target"], test["name"])] for test in requirement["tests"]],
        })
    for path in used:
        if path not in ledger["bindings"]:
            raise ValueError(f"unbound traceability source: {path}")
    if set(ledger["bindings"]) != used:
        raise ValueError("unused or missing source bindings")
    for path, expected in ledger["bindings"].items():
        if hashlib.sha256(source_path(root, path).read_bytes()).hexdigest() != expected:
            raise ValueError(f"stale reviewed source binding: {path}")
    mapped_features = {entry["feature"] for entry in ledger["requirements"]}
    feature_decomposition = []
    for feature in ledger["features"]:
        name = feature["feature"]
        selected = [row for row in rows if row["feature"] == name]
        expected = {row["id"] for row in selected}
        state = feature["decomposition"]
        if state == "complete" and not selected:
            raise ValueError(f"a declared complete feature has no requirement records: {name}")
        declared = feature["requirement_ids"]
        if len(declared) != len(set(declared)) or set(declared) != expected:
            raise ValueError(f"feature requirement accounting differs from records: {name}")
        if bool(selected) != (state in {"partial", "complete"}):
            raise ValueError(f"feature decomposition state disagrees with records: {name}")
        if state == "complete":
            if feature["remaining_scope"] or any(
                row["review"] != "approved" for row in selected
            ):
                raise ValueError(f"complete feature has unresolved scope or evidence: {name}")
        elif not feature["remaining_scope"] or any(
            not note.strip() for note in feature["remaining_scope"]
        ):
            raise ValueError(f"unfinished/excluded feature needs explicit remaining scope: {name}")
        feature_decomposition.append({
            "feature": name, "support_status": feature["support_status"],
            "decomposition": state, "requirement_ids": sorted(expected),
            "requirement_count": len(selected), "remaining_scope": feature["remaining_scope"],
            "evidence_strength_counts": {
                strength: sum(row["strength"] == strength for row in selected)
                for strength in ("functional_contract", "state_preservation", "safety_only", "runtime_divergence")
            },
        })
    decomposition_complete = all(
        entry["decomposition"] in {"complete", "excluded"} for entry in ledger["features"]
    ) and all(entry["review"] == "approved" for entry in ledger["requirements"])
    semantic_complete = decomposition_complete and all(
        entry["review"] == "approved" and not entry["gaps"]
        and entry["strength"] == "functional_contract" for entry in ledger["requirements"]
    )
    owner_confirmation = check_owner_review_confirmation(root, ledger)
    return {
        "schema_version": 1,
        "status": "references_valid_scope_complete" if semantic_complete else "references_valid_scope_incomplete",
        # Preserve the legacy full-strength gate while exposing the P04-only milestone.
        "normative_decomposition_complete": semantic_complete,
        "decomposition_review_complete": decomposition_complete,
        "semantic_claims_complete": semantic_complete,
        "scope_completeness_is_review_declared_not_proved": True,
        "owner_review_confirmation": owner_confirmation,
        "requirements": rows, "requirement_count": len(rows),
        "feature_count": len(features),
        "feature_decomposition": feature_decomposition,
        "standards": ledger["standards"],
        "source_consultations": consultations,
        "obligation_kind_counts": {
            kind: sum(row["obligation_kind"] == kind for row in rows)
            for kind in ("normative_rule", "project_profile_policy")
        },
        "evidence_strength_counts": {
            strength: sum(row["strength"] == strength for row in rows)
            for strength in ("functional_contract", "state_preservation", "safety_only", "runtime_divergence")
        },
        "decomposition_counts": {
            state: sum(feature["decomposition"] == state for feature in ledger["features"])
            for state in ("not_started", "partial", "complete", "excluded")
        },
        "features_without_requirement_records": sorted(set(features) - mapped_features),
        "verifier_report_checked": verifier_names is not None,
        "cargo_test_discovery_checked": discovered is not None,
        "release_gate": (
            "complete_scope_review_required" if not decomposition_complete
            else "semantic_claims_review_required" if not semantic_complete
            else "owner_release_review_required"
        ),
        "note": "Links and declared evidence strength are checked, not semantic truth. Report symbols do not establish report freshness: the caller must run pinned Verus on the same source, as CI does. Standard-to-model translation remains a review obligation.",
        "validation_readiness": assess(ledger, runtime, verifier_names, coverage),
    }


def discover_tests(root, ledger):
    return discover_cargo_tests(
        root, {test["target"] for req in ledger["requirements"] for test in req["tests"]},
    )


def enforce_completion(report, require_decomposition=False, require_complete=False):
    if require_decomposition and not report["decomposition_review_complete"]:
        raise ValueError("reviewed normative requirement decomposition remains incomplete")
    if require_complete and not report["semantic_claims_complete"]:
        raise ValueError("normative decomposition, semantic claims and approval remain incomplete")


def check_v01_scope(root, ledger, report):
    scope = ledger.get("release_scope")
    if not isinstance(scope, dict) or scope.get("profile") != "0.1-partial-verus":
        raise ValueError("0.1 scope requires the recorded owner partial-Verus release decision")
    try:
        approved_at = datetime.fromisoformat(scope["approved_at"])
    except (TypeError, ValueError):
        raise ValueError("0.1 release scope requires a timezone-qualified timestamp") from None
    if approved_at.tzinfo is None:
        raise ValueError("0.1 release scope requires a timezone-qualified timestamp")
    version = tomllib.loads((root / "Cargo.toml").read_text())["package"]["version"]
    if version != scope["crate_version"]:
        raise ValueError("0.1 release scope does not match the candidate crate version")
    if not (
        report["verifier_report_checked"] and report["cargo_test_discovery_checked"]
        and (report.get("owner_review_confirmation") or {}).get("normative_model_review_validated")
    ):
        raise ValueError("0.1 scope requires checked Verus, Cargo discovery and owner review evidence")
    return {
        "profile": scope["profile"], "crate_version": version,
        "owner_decision": scope["decision_reference"], "deferred_work": scope["deferred_work"],
        "requirement_coverage_policy_satisfied": True,
        "exhaustive_requirement_coverage": report["decomposition_review_complete"],
        "complete_functional_proof": report["semantic_claims_complete"],
        "remaining_quality_and_release_checks_required": True,
        "publication_performed": False,
    }


def check_development_scope(root, ledger, report):
    scope = ledger.get("development_scope")
    if not isinstance(scope, dict) or scope.get("profile") not in {"0.2-development", "0.3-development"}:
        raise ValueError("development scope requires a recorded supported development profile")
    package = tomllib.loads((root / "Cargo.toml").read_text())["package"]
    if package["version"] != scope["crate_version"] or package.get("publish") is not False:
        raise ValueError("development version must match its scope and keep publication disabled")
    if not report["verifier_report_checked"] or not report["cargo_test_discovery_checked"]:
        raise ValueError("development scope requires checked Verus and Cargo discovery evidence")
    return {
        **scope,
        "exhaustive_requirement_coverage": report["decomposition_review_complete"],
        "complete_functional_proof": report["semantic_claims_complete"],
        "remaining_quality_and_release_checks_required": True,
        "publication_performed": False,
    }


def check_current_scope(root, ledger, report):
    if ("development_scope" in ledger) == ("publication_scope" in ledger):
        raise ValueError("exactly one current development or publication scope is required")
    if "development_scope" in ledger:
        return check_development_scope(root, ledger, report)
    scope = ledger["publication_scope"]
    package = tomllib.loads((root / "Cargo.toml").read_text())["package"]
    if (
        scope["profile"] != "0.2-partial-verus"
        or scope["crate_version"] != package["version"]
        or package.get("publish") != ["crates-io"]
        or scope["release_branch"] != "release/0.2"
    ):
        raise ValueError("publication version, branch and crates.io-only target must match the approved scope")
    try:
        approved_at = datetime.fromisoformat(scope["approved_at"])
    except (TypeError, ValueError):
        raise ValueError("publication scope requires a timezone-qualified approval") from None
    if approved_at.tzinfo is None:
        raise ValueError("publication scope requires a timezone-qualified approval")
    if not report["verifier_report_checked"] or not report["cargo_test_discovery_checked"]:
        raise ValueError("publication scope requires checked Verus and Cargo discovery evidence")
    return {
        **scope,
        "exhaustive_requirement_coverage": report["decomposition_review_complete"],
        "complete_functional_proof": report["semantic_claims_complete"],
        "publication_performed": False,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--ledger", type=Path, default=ROOT / "validation/traceability/requirements.json")
    parser.add_argument("--verus-json", type=Path)
    parser.add_argument("--discover-tests", action="store_true")
    parser.add_argument("--standard-pdf", type=Path, help="Legacy hash check for the first declared standard.")
    parser.add_argument(
        "--standard-document", action="append", default=[], metavar="STANDARD_ID=PATH",
        help="Hash-check a named declared standard; repeat for multiple local documents.",
    )
    parser.add_argument("--require-complete", action="store_true")
    parser.add_argument(
        "--require-v01-scope", action="store_true",
        help="Require owner-approved partial-Verus 0.1 scope, checked proof and test discovery; not full coverage.",
    )
    parser.add_argument(
        "--require-development-scope", action="store_true",
        help="Require the unpublished development scope and actual proof/test discovery, not full coverage.",
    )
    parser.add_argument(
        "--require-current-scope", action="store_true",
        help="Require the branch's explicit development or publication scope and actual proof/test discovery.",
    )
    parser.add_argument(
        "--require-reviewed-decomposition", action="store_true",
        help="Require reviewed requirement enumeration without treating recorded semantic gaps as closed.",
    )
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--audit-packet", type=Path, help="Write an explicitly unreviewed, hash-bound review snapshot.")
    parser.add_argument("--audit-requirement", action="append", default=[], metavar="REQUIREMENT_ID")
    parser.add_argument("--check-audit-packet", type=Path)
    parser.add_argument("--expected-audit-sha256")
    parser.add_argument(
        "--source-review-pr", type=int,
        help="Authenticate a human source-snapshot disposition, never normative approval.",
    )
    parser.add_argument("--source-review-id", type=int, help="Explicit GitHub review ID; requires --source-review-pr.")
    arguments = parser.parse_args()
    if arguments.audit_packet and arguments.check_audit_packet:
        parser.error("audit packet creation and validation are separate operations")
    if bool(arguments.check_audit_packet) != bool(arguments.expected_audit_sha256):
        parser.error("packet validation requires a separately supplied --expected-audit-sha256")
    if bool(arguments.audit_packet) != bool(arguments.audit_requirement):
        parser.error("packet creation requires explicit --audit-requirement selections")
    if (arguments.source_review_pr is not None or arguments.source_review_id is not None) and (
        arguments.source_review_pr is None or arguments.source_review_id is None
        or not arguments.check_audit_packet
    ):
        parser.error("human source disposition needs both review IDs and a checked pinned packet")
    output_paths = {arguments.output.resolve(), arguments.output.with_suffix(".md").resolve()}
    inputs = [arguments.ledger, arguments.verus_json, arguments.standard_pdf, arguments.check_audit_packet]
    inputs.extend(
        Path(selection.split("=", 1)[1]) for selection in arguments.standard_document
        if "=" in selection
    )
    if any(path is not None and path.resolve() in output_paths for path in inputs):
        parser.error("reports must not overwrite input documents or evidence")
    if arguments.audit_packet or arguments.check_audit_packet:
        target = ROOT / "target"
        if target.is_symlink() or not all(path.is_relative_to(target.resolve()) for path in output_paths):
            parser.error("audit reports must stay under the local target directory")
        if arguments.audit_packet and (
            not arguments.audit_packet.resolve().is_relative_to(target.resolve())
            or arguments.audit_packet.resolve() in output_paths
        ):
            parser.error("create audit packets separately from reports under target/")
    if sum((arguments.require_v01_scope, arguments.require_development_scope,
            arguments.require_current_scope)) > 1:
        parser.error("development and publication scope profiles are mutually exclusive")
    if (arguments.require_complete or arguments.require_reviewed_decomposition
        or arguments.require_v01_scope or arguments.require_development_scope
        or arguments.require_current_scope) and (
        not arguments.verus_json or not arguments.discover_tests
    ):
        parser.error("completion gates need Verus evidence and Cargo test discovery")
    try:
        arguments.output.unlink(missing_ok=True)
        if arguments.output.with_suffix(".md") != arguments.output:
            arguments.output.with_suffix(".md").unlink(missing_ok=True)
        if arguments.audit_packet:
            arguments.audit_packet.unlink(missing_ok=True)
        ledger = read_evidence(arguments.ledger, "traceability")
        verifier = read_evidence(arguments.verus_json, "verus") if arguments.verus_json else None
        discovered = discover_tests(ROOT, ledger) if arguments.discover_tests else None
        report = check(ledger, ROOT, set(verifier["func-details"]) if verifier else None, discovered)
        report["model_audit_inventory"] = audit_inventory(ROOT, ledger)
        if arguments.require_v01_scope:
            report["scoped_release"] = check_v01_scope(ROOT, ledger, report)
        if arguments.require_development_scope:
            report["development_scope"] = check_development_scope(ROOT, ledger, report)
        if arguments.require_current_scope:
            report["current_scope"] = check_current_scope(ROOT, ledger, report)
        if arguments.audit_packet or arguments.check_audit_packet:
            ledger_path = arguments.ledger.resolve().relative_to(ROOT.resolve()).as_posix()
            if arguments.check_audit_packet:
                packet = load_json(arguments.check_audit_packet)
                check_audit_packet(
                    ROOT, ledger, packet,
                    arguments.expected_audit_sha256, ledger_path,
                )
                report["audit_packet_identity_checked"] = True
            else:
                packet = audit_packet(ROOT, ledger, arguments.audit_requirement, ledger_path)
                arguments.audit_packet.parent.mkdir(parents=True, exist_ok=True)
                arguments.audit_packet.write_text(json.dumps(packet, indent=2) + "\n", encoding="utf-8")
                report["audit_packet_sha256"] = packet["packet_sha256"]
        revision = subprocess.run(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True, check=True,
        ).stdout.strip()
        report["source_revision"] = revision
        if arguments.source_review_pr is not None:
            if subprocess.run(
                ["git", "status", "--porcelain", "--untracked-files=all"],
                cwd=ROOT, capture_output=True, text=True, check=True,
            ).stdout:
                raise ValueError("human source disposition requires a clean committed checkout")
            subprocess.run(
                ["git", "ls-files", "--error-unmatch", "--", *packet["source_bindings"]],
                cwd=ROOT, capture_output=True, text=True, check=True,
            )
            report["source_review_disposition"] = source_review_disposition(
                arguments.source_review_pr, arguments.source_review_id, revision,
                arguments.expected_audit_sha256,
            )
        report["ledger_sha256"] = hashlib.sha256(arguments.ledger.read_bytes()).hexdigest()
        report["normative_document_hash_checks"] = local_document_checks(
            ledger, arguments.standard_pdf, arguments.standard_document,
        )
        report["normative_document_hash_checked"] = all(report["normative_document_hash_checks"].values())
        arguments.output.parent.mkdir(parents=True, exist_ok=True)
        arguments.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        table = [
            "# Provisional normative traceability", "",
            f"Status: **{report['status']}**. No full-conformance or complete-proof claim.", "",
        ]
        readiness = report["validation_readiness"]
        table += [
            "## External-validation readiness gap assessment", "",
            "**Gap assessment only; readiness, semantic contract quality and complete production/reachability "
            "inventory are not qualified. No completion percentage is claimed.**", "",
            f"Selected explicit production declarations: **{readiness['code']['explicit_declarations']}**.",
            f"Local contract surfaces: `{json.dumps(readiness['code']['contract_surfaces'], sort_keys=True)}`.",
            f"Declarations with direct functional-evidence links: **{readiness['code']['with_direct_functional_evidence_links']}**.",
            f"No-local-postcondition accounting: `{json.dumps(readiness['code']['no_local_postcondition_by_accounting_status'], sort_keys=True)}`.",
            "A syntactic postcondition or direct link is not a functional-coverage judgment. "
            "Trait-owned contracts, generated code and call domains still need qualification.", "",
            f"Selected statement records: **{readiness['statements']['selected_records']}**; "
            "**exhaustive source-statement denominator: not established**.",
            "Per-declaration identities, shared verifier names, module distributions, explicit limitations "
            "and remaining feature worklists are in `validation_readiness` in the JSON report.", "",
            "| Module | Explicit declarations | Local postcondition | No local postcondition | Trait without local postcondition | Direct functional links |",
            "|---|---:|---:|---:|---:|---:|",
        ]
        for module in readiness["code"]["modules"]:
            surfaces = module["contract_surfaces"]
            table.append("| " + " | ".join(str(value) for value in (
                module["module"], module["explicit_declarations"],
                surfaces.get("local_postcondition_surface", 0),
                surfaces.get("no_local_postcondition_surface", 0),
                surfaces.get("trait_implementation_without_local_postcondition", 0),
                module["with_direct_functional_evidence_links"],
            )) + " |")
        table.append("")
        if "source_review_disposition" in report:
            disposition = report["source_review_disposition"]
            table += [
                f"Authenticated source-snapshot review: `{disposition['review_id']}` "
                f"by human user ID `{disposition['human_user_id']}`.",
                "**Not normative approval, document-processing authorization or a normative-only model freeze.**",
                "",
            ]
        table += [
            f"Reviewed decomposition complete: **{report['decomposition_review_complete']}**.",
            f"Semantic claims complete: **{report['semantic_claims_complete']}**.", "",
            "## Source-only author navigation (not independent normative approval)", "",
            f"Purpose classification complete: **{report['model_audit_inventory']['classification_complete']}**.",
            f"Purpose counts: `{json.dumps(report['model_audit_inventory']['classification_counts'], sort_keys=True)}`.",
            f"Inline structural coverage: `{json.dumps(report['model_audit_inventory']['inline_navigation'], sort_keys=True)}`.",
            "Exact members, justifications, unresolved purpose ambiguities, scope caveats and unchanged requirement links are in "
            "`model_audit_inventory` in the accompanying JSON report.", "",
            "| Requirement | Standard | Clause/context | Basis | Scope | Evidence strength | Open gaps |",
            "|---|---|---|---|---|---|---|",
        ]
        for row in report["requirements"]:
            clean = lambda value: str(value).replace("|", "\\|").replace("\n", " ")
            table.append("| " + " | ".join(clean(value) for value in (
                row["id"], row["standard"], row["clause"], row["obligation_kind"], row["title"],
                row["strength"], "; ".join(row["gaps"]),
            )) + " |")
        table += ["", "## Consulted source identity", ""]
        for consultation in report["source_consultations"]:
            table += [
                f"- {consultation['standard']}: {consultation['acquisition_url']}",
                f"  Document SHA-256: `{consultation['document_sha256']}`.",
                "  Consulted clause families: " + "; ".join(consultation["clause_families"]) + ".",
            ]
            if consultation["acquisition_kind"] == "official_package":
                table.append(f"  Package SHA-256: `{consultation['package_sha256']}`.")
            else:
                table.append("  Direct official PDF; no package identity is recorded.")
        table += [
            "", "Reference identities and declared scope are checked, not independent proof of standard correspondence.",
            "Project-profile policy rows use their cited standard only as context; selected feature/API policy hashes are checked separately and do not increase the normative-rule count.",
        ]
        table += [
            "", "## Feature decomposition accounting", "",
            "Selected records are not an exhaustive normative denominator. Remaining scope is a worklist, not a completeness certificate.",
            "", "| Feature | State | Selected records | Remaining scope |",
            "|---|---|---:|---|",
        ]
        for feature in report["feature_decomposition"]:
            table.append("| " + " | ".join(clean(value) for value in (
                feature["feature"], feature["decomposition"], feature["requirement_count"],
                "; ".join(feature["remaining_scope"]),
            )) + " |")
        def link(location, name):
            line = f"#L{location['line']}" if "line" in location else ""
            return f"[`{name}`](https://github.com/microsoft/jbig2_rs/blob/{revision}/{location['file']}{line})"
        for row in report["requirements"]:
            table += [
                "", f"## {row['id']}: {row['title']}", "",
                f"Standard: {row['standard']}.",
                f"Clause: {row['clause']}; review: {row['review']}; strength: {row['strength']}.",
                f"Obligation basis: {row['obligation_kind']}.",
                "", row["scope_of_formal_result"], "",
                "Runtime: " + ", ".join(link(item, item["name"]) for item in row["runtime_locations"]),
                "Models (definitions, not independent correctness theorems): "
                + (", ".join(link(item, item["name"]) for item in row["models"]) or "none"),
                "Proof support: "
                + (", ".join(link(item, item["name"]) for item in row["proof_macros"]) or "runtime declaration/contract only"),
                "Tests: " + ", ".join(link(item, f"{item['target']}::{item['name']}") for item in row["tests"]),
                "Assumptions: " + "; ".join(row["assumptions"]),
                "Open gaps: " + ("; ".join(row["gaps"]) or "none within this recorded property scope"),
            ]
            if row["obligation_kind"] == "project_profile_policy":
                table.append(
                    f"Policy basis: `config/support-matrix.json` {row['policy_source']} "
                    f"(feature `{row['feature']}`), reviewed source SHA-256 `{row['policy_row_sha256']}`."
                )
        table += ["", report["note"]]
        arguments.output.with_suffix(".md").write_text("\n".join(table) + "\n", encoding="utf-8")
        summary = {key: report[key] for key in (
            "status", "requirement_count", "feature_count", "verifier_report_checked",
            "cargo_test_discovery_checked", "normative_document_hash_checked", "normative_document_hash_checks",
            "decomposition_review_complete", "semantic_claims_complete",
        )}
        if "source_review_disposition" in report:
            summary["source_review_disposition"] = report["source_review_disposition"]
        if "scoped_release" in report:
            summary["scoped_release"] = report["scoped_release"]
        if "development_scope" in report:
            summary["development_scope"] = report["development_scope"]
        if "current_scope" in report:
            summary["current_scope"] = report["current_scope"]
        print(json.dumps(summary, indent=2))
        enforce_completion(
            report, arguments.require_reviewed_decomposition, arguments.require_complete,
        )
        return 0
    except (
        OSError, ValueError, KeyError, TypeError, ValidationError,
        subprocess.CalledProcessError, subprocess.TimeoutExpired,
    ) as error:
        print(f"traceability validation failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
