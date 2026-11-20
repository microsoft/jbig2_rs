"""Reconcile actual expanded project Verus functions with native source identities."""

from collections import Counter, defaultdict
import hashlib
import json
from pathlib import PurePosixPath
import re

from check_traceability import runtime_inventory
from implementation_identity import function_name
from compiler_inventory import source_datatypes
from rust_source import contract_sections, items, parse, source_modules


TOKEN = re.compile(r'"(?:\\.|[^"\\])*"|[()]|[^\s()"]+')
SPAN = re.compile(r"(.+):(\d+):(\d+): (\d+):(\d+) \(#(\d+)\)")
FUNCTION = re.compile(r'^\(@ "(?:\\.|[^"\\])*"\s+\(Function\b')
PREFIX = "jbig2_rs_production::"


def parse_form(text):
    stack, forms, previous = [], [], 0
    for match in TOKEN.finditer(text):
        if text[previous:match.start()].strip():
            raise ValueError("unparsed expanded VIR token")
        previous = match.end()
        token = match.group()
        if token == "(":
            node = []
            (stack[-1] if stack else forms).append(node)
            stack.append(node)
        elif token == ")":
            if not stack:
                raise ValueError("unbalanced expanded VIR close")
            stack.pop()
        else:
            if not stack:
                raise ValueError("expanded VIR atom outside a form")
            stack[-1].append(token)
    if text[previous:].strip() or stack or len(forms) != 1:
        raise ValueError("truncated or duplicate expanded VIR form")
    return forms[0]


def fields(node, offset=1):
    if not isinstance(node, list) or (len(node) - offset) % 2:
        raise ValueError("unrecognized expanded VIR field shape")
    keys = node[offset::2]
    if any(not isinstance(key, str) or not key.startswith(":") for key in keys) \
            or len(set(keys)) != len(keys):
        raise ValueError("ambiguous expanded VIR named fields")
    return dict(zip(keys, node[offset + 1::2]))

def own_datatype(node):
    if not isinstance(node, list):
        return None
    if node[:2] == ["Dt", "Path"] and len(node) == 3 \
            and isinstance(node[2], str) and node[2].startswith(PREFIX):
        return node[2].rsplit("::", 1)[-1]
    for child in node:
        found = own_datatype(child)
        if found:
            return found
    return None

def lexical_module(source):
    path = PurePosixPath(source["file"]).relative_to("src")
    parts = path.with_suffix("").parts
    if path.name == "mod.rs":
        parts = parts[:-1]
    physical = "::".join(parts)
    tail = (source["owner"] + "::" if source["owner"] else "") + source["name"]
    prefix = source["id"].removesuffix(tail).rstrip(":")
    if not prefix.startswith(physical):
        raise ValueError("source identity lacks its physical module prefix")
    inline = prefix[len(physical):].strip(":")
    return "::".join(part for part in (source["file_module"], inline) if part)


def project_functions(root, text):
    records, paths = [], set()
    forms = [chunk.strip() for chunk in text.strip().split("\n\n") if chunk.strip()]
    if not forms or not text.startswith("(@ "):
        raise ValueError("unrecognized expanded VIR stream")
    for chunk in forms:
        form = parse_form(chunk)
        if not FUNCTION.match(chunk):
            continue
        if len(form) != 3 or form[0] != "@" or form[2][:1] != ["Function"]:
            raise ValueError("invalid expanded function wrapper")
        data = fields(form[2])
        name = fields(data[":name"])[":path"]
        if not isinstance(name, str):
            raise ValueError("expanded function lacks a path")
        if not name.startswith(PREFIX):
            continue
        if name in paths:
            raise ValueError("duplicate expanded project function path")
        paths.add(name)
        location = SPAN.fullmatch(json.loads(form[1]))
        if location is None:
            raise ValueError("project expanded function lacks an exact source span")
        filename, line, column, last_line, last_column, expansion = location.groups()
        path = (root / "verification" / filename).resolve()
        if not path.is_relative_to(root.resolve()) or path.suffix != ".rs" or not path.is_file():
            raise ValueError("project expanded function has an external or missing source owner")
        relative = path.relative_to(root.resolve()).as_posix()
        if relative.split("/", 1)[0] not in {"src", "specs", "proofs"}:
            raise ValueError("project expanded function has an unqualified source domain")
        begin = (int(line), int(column))
        end = (int(last_line), int(last_column))
        if min(*begin, *end) < 1 or end < begin or end[0] > len(path.read_text().splitlines()):
            raise ValueError("expanded function has an invalid source coordinate range")
        mode = data[":mode"]
        if mode not in {"Exec", "Spec", "Proof"}:
            raise ValueError("unqualified project expanded execution mode")
        if not isinstance(data[":owning_module"], str):
            raise ValueError("expanded function lacks a module owner")
        kind = data[":kind"]
        if kind[:1] != ["FunctionKind"] or kind[1] not in {
            "Static", "TraitMethodDecl", "TraitMethodImpl",
        }:
            raise ValueError("unqualified project expanded callable kind")
        ensure = data[":ensure"]
        if len(ensure) != 3 or ensure[0] != "tuple" or any(
            not isinstance(value, list) for value in ensure[1:]
        ):
            raise ValueError("unqualified expanded postcondition shape")
        attributes = fields(data[":attrs"])
        if attributes[":is_external_body"] not in {"true", "false"}:
            raise ValueError("unqualified expanded external-body status")
        row = {
            "verus_path": name, "file": relative, "line": int(line), "column": int(column),
            "end": [int(last_line), int(last_column)], "macro_expansion": int(expansion),
            "file_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "mode": mode, "kind": kind[1],
            "owning_module": data[":owning_module"],
            "trait": fields(kind, 2).get(":trait_path") if kind[1] == "TraitMethodImpl" else None,
            "body_present": data[":body"] != "None",
            "external_body": attributes[":is_external_body"] == "true",
            "item_kind": data.get(":item_kind", ["ItemKind", "Function"]),
            "requires_entries": len(data[":require"]),
            "postcondition_entries": sum(len(value) for value in ensure[1:]),
            "signature": {
                key.removeprefix(":"): data[key] for key in (
                    ":typ_params", ":typ_bounds", ":params", ":ret",
                )
            },
            "contract_sha256": hashlib.sha256(json.dumps({
                key: data[key] for key in (":require", ":ensure", ":returns", ":decrease")
            }, separators=(",", ":")).encode()).hexdigest(),
            "expanded_form_sha256": hashlib.sha256(chunk.encode()).hexdigest(),
        }
        row["canonical_verifier_name"] = name
        if "::impl&%" in name:
            target = None
            if kind[1] == "TraitMethodImpl":
                target = own_datatype(fields(kind, 2)[":trait_typ_args"][0])
            if target is None:
                for parameter in data[":params"]:
                    parameter = fields(parameter[2])
                    if parameter[":name"][1] == '"self"':
                        target = own_datatype(parameter[":typ"])
                        break
            if target is not None:
                row["canonical_verifier_name"] = name.rsplit("::impl&%", 1)[0] + "::" + target \
                    + "::" + name.rsplit("::", 1)[-1]
        records.append(row)
    if not records:
        raise ValueError("expanded inventory has no project functions")
    return records


def executable_constants(root):
    constants = {}

    def walk(container, file, module):
        for node, attributes in items(container):
            if node.type == "mod_item":
                body = node.child_by_field_name("body")
                if body is not None:
                    name = node.child_by_field_name("name").text.decode()
                    walk(body, file, "::".join(part for part in (module, name) if part))
            if node.type != "const_item":
                continue
            record = {"attributes": [attribute.text.decode() for attribute in attributes]}
            if not contract_sections(record, "ensures"):
                continue
            name = node.child_by_field_name("name").text.decode()
            key = (file, node.start_point.row + 1, node.start_point.column + 1, name)
            if key in constants:
                raise ValueError("duplicate executable constant source identity")
            constants[key] = PREFIX + "::".join(part for part in (module, name) if part)

    for file, module in source_modules(root).items():
        walk(parse((root / file).read_bytes()), file, module)
    return constants


def reconcile(root, text, native, whole, coverage):
    result = whole["verification-results"]
    if (
        result["success"] is not True or result["errors"] != 0
        or result["is-verifying-entire-crate"] is not True or not whole["func-details"]
    ):
        raise ValueError("expanded reconciliation requires actual successful unfiltered whole evidence")
    runtime = runtime_inventory(root)
    accounting = {row["id"]: row for row in coverage["functions"]}
    if len(accounting) != len(coverage["functions"]) or set(accounting) != set(runtime):
        raise ValueError("expanded inventory must use complete explicit production accounting")
    if any(accounting[identity]["file"] != row["file"]
           or accounting[identity]["file_sha256"] != row["file_sha256"]
           for identity, row in runtime.items()):
        raise ValueError("stale expanded production accounting source identity")
    native_records = native["records"] if native is not None else []
    if native is not None:
        native_source = {row["source_id"]: row for row in native_records
                         if row["origin"] == "explicit_source_declaration"}
        if set(native_source) != set(runtime):
            raise ValueError("native compiler and explicit source identities differ")
    source_keys = defaultdict(list)
    derives = defaultdict(list)
    for identity, row in runtime.items():
        source_keys[(row["file"], row["line"], row["name"])].append(identity)
    for row in native_records:
        if row["origin"] == "compiler_derived_method":
            span = row["source_span"]
            derives[(span["filename"], *span["begin"], row["name"])].append(row)
    if native is None:
        _, source_derives, _ = source_datatypes(root)
        for file, line, column, method, target in source_derives:
            if method != "clone":
                continue
            derives[(file, line, column, method)].append({
                "id": f"derive:{file}:{line}:{column}:clone", "impl_trait": "Clone",
            })
    records = project_functions(root, text)
    constants = executable_constants(root)
    matched, derived, matched_constants = set(), set(), set()
    for row in records:
        row["native_id"] = None
        row["qualification"] = "proof_only_source_function; not a native callable claim"
        if row["mode"] != "Exec":
            continue
        name = row["verus_path"].rsplit("::", 1)[-1]
        if row["item_kind"] == ["ItemKind", "Const"]:
            key = (row["file"], row["line"], row["column"], name)
            expected = constants.get(key)
            if (
                expected is None or key in matched_constants
                or row["verus_path"] != expected
                or row["owning_module"].rstrip(":") != expected.rsplit("::", 1)[0]
                or row["kind"] != "Static" or row["external_body"] or not row["body_present"]
                or expected not in whole["func-details"]
            ):
                raise ValueError("unqualified expanded executable constant initializer")
            row["qualification"] = "exact_source_const_initializer_and_whole_name; not a native callable"
            matched_constants.add(key)
            continue
        if row["item_kind"] != ["ItemKind", "Function"]:
            raise ValueError("unqualified expanded executable item kind")
        choices = source_keys[(row["file"], row["line"], name)]
        if len(choices) > 1:
            raise ValueError("expanded span aliases explicit source declarations")
        if choices:
            identity = choices[0]
            if identity in matched:
                raise ValueError("duplicate expanded explicit implementation")
            source = runtime[identity]
            expected_module = (PREFIX + lexical_module(source)).rstrip(":")
            if row["owning_module"].rstrip(":") != expected_module:
                raise ValueError(
                    f"expanded implementation module differs for {identity}: "
                    f"{row['owning_module']} != {expected_module}"
                )
            if source["impl_trait"] is None:
                if row["kind"] != "Static":
                    raise ValueError("expanded inherent/free function has a different callable kind")
            elif row["kind"] != "TraitMethodImpl" or row["trait"].rsplit("::", 1)[-1] != \
                    source["impl_trait"].split("<", 1)[0].rsplit("::", 1)[-1]:
                raise ValueError("expanded trait implementation has a different trait owner")
            if accounting[identity]["status"] != "verified" or not source["verification_requested"]:
                raise ValueError("expanded source body conflicts with reviewed production boundary")
            canonical = PREFIX + function_name(source)
            if canonical not in whole["func-details"]:
                raise ValueError("expanded explicit body lacks current whole verifier name evidence")
            if not row["body_present"] or row["external_body"]:
                raise ValueError("verified expanded explicit function has no checked implementation body")
            row.update(native_id=identity, canonical_verifier_name=canonical,
                       qualification="exact_expanded_source_identity_and_whole_name; semantic ownership separate")
            matched.add(identity)
            continue
        choices = derives[(row["file"], row["line"], row["column"], name)]
        if len(choices) != 1:
            raise ValueError("unmapped expanded executable needs explicit source-domain qualification")
        source = choices[0]
        if source["id"] in derived or row["trait"] != "core::clone::Clone":
            raise ValueError("unqualified or duplicate proof-mode native derive")
        if not row["body_present"] or row["external_body"]:
            raise ValueError("expanded native derive has no checked body")
        row.update(native_id=source["id"], qualification="actual_expanded_native_clone; result presence not quality")
        derived.add(source["id"])
    expected = {identity for identity, row in accounting.items() if row["status"] == "verified"}
    if matched != expected:
        raise ValueError("expanded inventory omits verified explicit production implementations")
    if matched_constants != set(constants):
        raise ValueError("expanded inventory omits verified executable constant initializers")
    expected_derives = {row["id"] for rows in derives.values() for row in rows
                        if row["impl_trait"] == "Clone"}
    if derived != expected_derives:
        raise ValueError("expanded inventory omits native Clone derivations")
    whole_names = {name for name in whole["func-details"] if name.startswith(PREFIX)}
    expanded_names = {row["canonical_verifier_name"] for row in records}
    if not whole_names <= expanded_names:
        raise ValueError("expanded inventory omits project identities recorded by the whole verifier")
    absent = [row for row in native_records if row["origin"] in {
        "explicit_source_declaration", "compiler_derived_method",
    } and row["id"] not in matched | derived]
    return {
        "schema_version": 1, "status": "expanded_project_function_inventory_reconciled",
        "counts": {
            "project_functions": len(records),
            "modes": dict(sorted(Counter(row["mode"] for row in records).items())),
            "matched_verified_explicit": len(matched),
            "matched_source_clones": len(derived),
            "matched_exec_const_initializers": len(matched_constants),
            "native_function_items_not_in_expanded_exec": len(absent) if native is not None else None,
            "whole_project_names_bound": len(whole_names),
        },
        "records": records,
        "native_function_items_not_in_expanded_exec": [{
            "id": row["id"], "origin": row["origin"], "source_span": row["source_span"],
            "impl_trait": row["impl_trait"],
            "accounting_status": accounting[row["id"]]["status"] if row["id"] in accounting else None,
            "qualification": "native declaration remains visible; no expanded proof credit",
        } for row in absent],
        "ready_for_external_validation": False,
        "native_profile_reconciled": native is not None,
        "generated_contract_quality_qualified": False,
        "public_entry_reachability_qualified": False,
        "claim_boundary": (
            "Original expanded function forms/signatures/contract counts bound to native source "
            "and current whole evidence. Trait-provider contracts may be injected at a later "
            "verification stage; absent expanded local ensures is not absence of effective "
            "contract. Shared-name semantic ownership remains separately qualified. Native-only "
            "formatting/equality/defaults, nested closures, ADT constructors/marker/CTFE stages, "
            "proof-only executables, full platform/graph/domain/semantic quality remain separate."
        ),
    }
