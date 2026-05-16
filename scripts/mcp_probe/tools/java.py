"""Java probe inputs for scripts/mcp_probe/probe.py.

Mirrors the shape of `tools/rust.py`: returns `{tool_name: [args_dict, ...]}`
with exactly 5 query variants per tool. Paths default to the Maven/Gradle
layout (`src/main/java`, `src/test/java`) plus the flat `src` / `test`
fallback that simpler projects use; base-class lookups go through the usual
`Object` / `Exception` / `Runnable` / `Comparable` / `Serializable` suspects
so most JVM codebases will exercise the inheritance and implements tools.
"""

DEFAULT_NAMES = ["main", "toString", "equals", "hashCode", "run"]
DEFAULT_BASES = ["Object", "Exception", "Runnable", "Comparable", "Serializable"]
DEFAULT_TYPE_NAMES = ["String", "Object", "List", "Map", "Optional"]
DEFAULT_TASKS = [
    "error handling",
    "logging",
    "dependency injection",
    "async runtime",
    "serialization",
]
DEFAULT_PATHS = ["src/main/java", "src/test/java", "src", "test", "lib"]


def probes_for(d):
    ids = d["ids"]
    qns = d["qnames"]
    names = d["names"] or DEFAULT_NAMES
    files = d["files"]
    return {
        # core
        "tokensave_search":            [{"query": q} for q in
                                        ["main", "toString", "equals", "Exception", "Optional"]],
        "tokensave_context":           [{"task": t} for t in DEFAULT_TASKS],
        "tokensave_node":              [{"node_id": i} for i in ids],
        "tokensave_by_qualified_name": [{"qualified_name": q} for q in qns],
        "tokensave_signature":         [{"node_id": i} for i in ids],
        "tokensave_body":              [{"symbol": n} for n in names],
        # traversal
        "tokensave_callers":           [{"node_id": i} for i in ids],
        "tokensave_callees":           [{"node_id": i} for i in ids],
        "tokensave_callers_for":       [{"node_ids": [i]} for i in ids],
        "tokensave_impls":             [{"name": n} for n in DEFAULT_BASES],
        "tokensave_derives":           [{"qualified_name": q} for q in qns],
        "tokensave_type_hierarchy":    [{"node_id": i} for i in ids],
        "tokensave_similar":           [{"symbol": n} for n in names],
        "tokensave_rank":              [{"edge_kind": k} for k in
                                        ["implements", "extends", "calls", "uses", "contains"]],
        "tokensave_impact":            [{"node_id": i} for i in ids],
        "tokensave_rename_preview":    [{"node_id": i, "new_name": "renamed"} for i in ids],
        # analysis (whole-DB sweeps)
        "tokensave_hotspots":          [{}, {"limit": 10}, {"path": "src/main/java"}, {"path": "src/test/java"}, {"path": "src"}],
        "tokensave_complexity":        [{}, {"limit": 10}, {"path": "src/main/java"}, {"path": "src/test/java"}, {"path": "src"}],
        "tokensave_dead_code":         [{}, {"limit": 10}, {"include_public": False},
                                        {"path": "src/main/java"}, {"path": "src"}],
        "tokensave_circular":          [{}, {"limit": 5}, {"path": "src/main/java"}, {"path": "src/test/java"}, {"path": "src"}],
        "tokensave_doc_coverage":      [{}, {"limit": 10}, {"path": "src/main/java"}, {"path": "src"}, {"path": files[0]}],
        "tokensave_god_class":         [{}, {"limit": 10}, {"path": "src/main/java"}, {"path": "src/test/java"}, {"path": "src"}],
        "tokensave_dependency_depth":  [{}, {"limit": 10}, {"path": "src/main/java"}, {"path": "src/test/java"}, {"path": "src"}],
        "tokensave_inheritance_depth": [{}, {"limit": 10}, {"path": "src/main/java"}, {"path": "src/test/java"}, {"path": "src"}],
        "tokensave_distribution":      [{}, {"path": "src/main/java"}, {"path": "src/test/java"}, {"path": "src"}, {"path": "test"}],
        "tokensave_gini":              [{}, {"path": "src/main/java"}, {"path": "src/test/java"}, {"path": "src"}, {"path": "test"}],
        "tokensave_largest":           [{}, {"limit": 10}, {"path": "src/main/java"}, {"path": "src/test/java"}, {"path": "src"}],
        "tokensave_recursion":         [{}, {"limit": 10}, {"path": "src/main/java"}, {"path": "src/test/java"}, {"path": "src"}],
        "tokensave_coupling":          [{}, {"path": "src/main/java"}, {"path": "src/test/java"}, {"path": "src"}, {"path": files[0]}],
        "tokensave_dsm":               [{}, {"path": "src/main/java"}, {"path": "src/test/java"}, {"path": "src"}, {"path": "test"}],
        "tokensave_module_api":        [{"path": f} for f in files],
        "tokensave_simplify_scan":     [{"files": [f]} for f in files],
        "tokensave_unused_imports":    [{}, {"limit": 10}, {"path": "src/main/java"}, {"path": "src/test/java"}, {"path": "src"}],
        "tokensave_test_map":          [{"file": f} for f in files],
        "tokensave_test_risk":         [{}, {"limit": 10}, {"path": "src/main/java"}, {"path": "src/test/java"}, {"path": "src"}],
        "tokensave_todos":             [{}, {"limit": 10}, {"path": "src/main/java"}, {"path": "src/test/java"}, {"path": "src"}],
        "tokensave_files":             [{}, {"path": "src/main/java"}, {"path": "src/test/java"}, {"path": "src"}, {"path": "test"}],
        "tokensave_status":            [{}] * 5,
        "tokensave_health":            [{}] * 5,
        "tokensave_diagnose":          [{"cargo_output": "src/main/java/Foo.java:1: error: cannot find symbol\n    bar();\n    ^\n  symbol:   method bar()"}] * 5,
        # port
        "tokensave_port_status":       [
            {"source_dir": "src/main/java", "target_dir": "src/test/java"},
            {"source_dir": "src", "target_dir": "test"},
            {"source_dir": "src/main/java", "target_dir": "src/main/java"},
            {"source_dir": "src/test/java", "target_dir": "src/main/java"},
            {"source_dir": "src", "target_dir": "lib"},
        ],
        "tokensave_port_order":        [{"source_dir": d} for d in DEFAULT_PATHS],
        # git/branch
        "tokensave_branch_list":       [{}] * 5,
        "tokensave_branch_diff":       [
            {"base": "master", "head": "master"},
            {"base": "main", "head": "main"},
            {},
            {"base": "HEAD", "head": "HEAD"},
            {"base": "HEAD~1", "head": "HEAD"},
        ],
        "tokensave_branch_search":     [
            {"branch": "main", "query": "main"},
            {"branch": "master", "query": "main"},
            {"branch": "main", "query": "toString"},
            {"branch": "main", "query": "Exception"},
            {"branch": "master", "query": "Optional"},
        ],
        "tokensave_changelog":         [
            {"from_ref": "HEAD~10", "to_ref": "HEAD"},
            {"from_ref": "HEAD~5", "to_ref": "HEAD"},
            {"from_ref": "HEAD~1", "to_ref": "HEAD"},
            {"from_ref": "HEAD~20", "to_ref": "HEAD~5"},
            {"from_ref": "HEAD~3", "to_ref": "HEAD"},
        ],
        "tokensave_pr_context":        [{}, {"base_ref": "main"}, {"base_ref": "master"},
                                        {"base_ref": "HEAD~5"}, {"base_ref": "HEAD~1"}],
        "tokensave_diff_context":      [{"files": [f]} for f in files],
        "tokensave_commit_context":    [{}, {"commit": "HEAD"}, {"commit": "HEAD~1"},
                                        {"commit": "HEAD~5"}, {"commit": "HEAD~10"}],
        "tokensave_affected":          [{"files": [f]} for f in files],
    }
