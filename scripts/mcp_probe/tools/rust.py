"""Rust probe inputs for scripts/mcp_probe/probe.py.

`probes_for(discovered)` returns a `{tool_name: [args_dict, ...]}` mapping.
Each tool gets exactly 5 query variants. `discovered` is the dict produced
by probe.py's discover() step — it contains real node ids, qualified names,
names, and file paths harvested from the repo via tokensave_search /
tokensave_node, so the per-node tools get plausible inputs.
"""

DEFAULT_NAMES = ["main", "new", "process", "format", "drop"]
DEFAULT_IMPLS = ["Display", "Debug", "Default", "Clone", "Drop"]
DEFAULT_TYPES = ["Error", "Iterator", "Future", "Display", "Default"]
DEFAULT_TASKS = [
    "error handling",
    "logging",
    "config parsing",
    "async runtime",
    "serialization",
]


def probes_for(d):
    ids = d["ids"]
    qns = d["qnames"]
    names = d["names"] or DEFAULT_NAMES
    files = d["files"]
    return {
        # core
        "tokensave_search":            [{"query": q} for q in ["main", "new", "process", "Config", "Error"]],
        "tokensave_context":           [{"task": t} for t in DEFAULT_TASKS],
        "tokensave_node":              [{"node_id": i} for i in ids],
        "tokensave_by_qualified_name": [{"qualified_name": q} for q in qns],
        "tokensave_signature":         [{"node_id": i} for i in ids],
        "tokensave_body":              [{"symbol": n} for n in names],
        # traversal
        "tokensave_callers":           [{"node_id": i} for i in ids],
        "tokensave_callees":           [{"node_id": i} for i in ids],
        "tokensave_callers_for":       [{"node_ids": [i]} for i in ids],
        "tokensave_impls":             [{"name": n} for n in DEFAULT_IMPLS],
        "tokensave_derives":           [{"qualified_name": q} for q in qns],
        "tokensave_type_hierarchy":    [{"node_id": i} for i in ids],
        "tokensave_similar":           [{"symbol": n} for n in names],
        "tokensave_rank":              [{"edge_kind": k} for k in
                                        ["implements", "extends", "calls", "uses", "contains"]],
        "tokensave_impact":            [{"node_id": i} for i in ids],
        "tokensave_rename_preview":    [{"node_id": i, "new_name": "renamed"} for i in ids],
        # analysis (whole-DB sweeps)
        "tokensave_hotspots":          [{}, {"limit": 10}, {"path": "src"}, {"path": "crates"}, {"path": "tests"}],
        "tokensave_complexity":        [{}, {"limit": 10}, {"path": "src"}, {"path": "crates"}, {"path": "tests"}],
        "tokensave_dead_code":         [{}, {"limit": 10}, {"include_public": False},
                                        {"path": "src"}, {"path": "crates"}],
        "tokensave_circular":          [{}, {"limit": 5}, {"path": "src"}, {"path": "crates"}, {"path": "tests"}],
        "tokensave_doc_coverage":      [{}, {"limit": 10}, {"path": "src"}, {"path": "crates"}, {"path": files[0]}],
        "tokensave_god_class":         [{}, {"limit": 10}, {"path": "src"}, {"path": "crates"}, {"path": "tests"}],
        "tokensave_dependency_depth":  [{}, {"limit": 10}, {"path": "src"}, {"path": "crates"}, {"path": "tests"}],
        "tokensave_inheritance_depth": [{}, {"limit": 10}, {"path": "src"}, {"path": "crates"}, {"path": "tests"}],
        "tokensave_distribution":      [{}, {"path": "src"}, {"path": "crates"}, {"path": "tests"}, {"path": "benches"}],
        "tokensave_gini":              [{}, {"path": "src"}, {"path": "crates"}, {"path": "tests"}, {"path": "benches"}],
        "tokensave_largest":           [{}, {"limit": 10}, {"path": "src"}, {"path": "crates"}, {"path": "tests"}],
        "tokensave_recursion":         [{}, {"limit": 10}, {"path": "src"}, {"path": "crates"}, {"path": "tests"}],
        "tokensave_coupling":          [{}, {"path": "src"}, {"path": "crates"}, {"path": "tests"}, {"path": files[0]}],
        "tokensave_dsm":               [{}, {"path": "src"}, {"path": "crates"}, {"path": "tests"}, {"path": "benches"}],
        "tokensave_module_api":        [{"path": f} for f in files],
        "tokensave_simplify_scan":     [{"files": [f]} for f in files],
        "tokensave_unused_imports":    [{}, {"limit": 10}, {"path": "src"}, {"path": "crates"}, {"path": "tests"}],
        "tokensave_test_map":          [{"file": f} for f in files],
        "tokensave_test_risk":         [{}, {"limit": 10}, {"path": "src"}, {"path": "crates"}, {"path": "tests"}],
        "tokensave_todos":             [{}, {"limit": 10}, {"path": "src"}, {"path": "crates"}, {"path": "tests"}],
        "tokensave_files":             [{}, {"path": "src"}, {"path": "crates"}, {"path": "tests"}, {"path": "benches"}],
        "tokensave_status":            [{}] * 5,
        "tokensave_health":            [{}] * 5,
        "tokensave_diagnose":          [{"cargo_output": "warning: unused variable `x` --> src/lib.rs:1:5"}] * 5,
        # port (recently fixed)
        "tokensave_port_status":       [
            {"source_dir": "src", "target_dir": "tests"},
            {"source_dir": "crates", "target_dir": "tests"},
            {"source_dir": "src", "target_dir": "src"},
            {"source_dir": "src", "target_dir": "benches"},
            {"source_dir": "tests", "target_dir": "src"},
        ],
        "tokensave_port_order":        [{"source_dir": d} for d in
                                        ["src", "crates", "tests", "benches", "src/lib.rs"]],
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
            {"branch": "master", "query": "main"},
            {"branch": "main", "query": "main"},
            {"branch": "master", "query": "new"},
            {"branch": "master", "query": "Error"},
            {"branch": "main", "query": "Config"},
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
