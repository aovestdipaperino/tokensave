//! MCP tool definitions (JSON Schema descriptors).
//!
//! Each `def_*` function returns a `ToolDefinition` with the tool name,
//! description, and JSON Schema for its input parameters.

use serde_json::json;

use super::ToolDefinition;

/// Returns the list of all tool definitions exposed by this MCP server.
pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    let definitions = vec![
        def_search(),
        def_context(),
        def_callers(),
        def_callees(),
        def_impact(),
        def_node(),
        def_status(),
        def_files(),
        def_affected(),
        def_dead_code(),
        def_diff_context(),
        def_module_api(),
        def_circular(),
        def_hotspots(),
        def_similar(),
        def_rename_preview(),
        def_unused_imports(),
        def_rank(),
        def_largest(),
        def_coupling(),
        def_inheritance_depth(),
        def_distribution(),
        def_recursion(),
        def_complexity(),
        def_doc_coverage(),
        def_god_class(),
        def_changelog(),
        def_port_status(),
        def_port_order(),
    ];
    debug_assert!(!definitions.is_empty(), "get_tool_definitions returned empty list");
    debug_assert!(definitions.iter().all(|d| d.name.starts_with("tokensave_")),
        "all tool definitions must have 'tokensave_' prefix");
    definitions
}


fn def_search() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_search".to_string(),
        description: "Search for symbols (functions, structs, traits, etc.) in the code graph by name or keyword.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query string to match against symbol names"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of results to return (default: 10)"
                }
            },
            "required": ["query"]
        }),
    }
}

fn def_context() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_context".to_string(),
        description: "Build an AI-ready context for a task description. Returns relevant symbols, relationships, and code snippets.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "Natural language description of the task or question"
                },
                "max_nodes": {
                    "type": "number",
                    "description": "Maximum number of symbols to include (default: 20)"
                }
            },
            "required": ["task"]
        }),
    }
}

fn def_callers() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_callers".to_string(),
        description: "Find all callers of a given node (function, method, etc.) up to a specified depth.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "node_id": {
                    "type": "string",
                    "description": "The unique node ID to find callers for"
                },
                "max_depth": {
                    "type": "number",
                    "description": "Maximum traversal depth (default: 3)"
                }
            },
            "required": ["node_id"]
        }),
    }
}

fn def_callees() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_callees".to_string(),
        description: "Find all callees of a given node (function, method, etc.) up to a specified depth.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "node_id": {
                    "type": "string",
                    "description": "The unique node ID to find callees for"
                },
                "max_depth": {
                    "type": "number",
                    "description": "Maximum traversal depth (default: 3)"
                }
            },
            "required": ["node_id"]
        }),
    }
}

fn def_impact() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_impact".to_string(),
        description: "Compute the impact radius of a node: all symbols that directly or indirectly depend on it.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "node_id": {
                    "type": "string",
                    "description": "The unique node ID to compute impact for"
                },
                "max_depth": {
                    "type": "number",
                    "description": "Maximum traversal depth (default: 3)"
                }
            },
            "required": ["node_id"]
        }),
    }
}

fn def_node() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_node".to_string(),
        description: "Retrieve detailed information about a single node by its ID.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "node_id": {
                    "type": "string",
                    "description": "The unique node ID to retrieve"
                }
            },
            "required": ["node_id"]
        }),
    }
}

fn def_status() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_status".to_string(),
        description: "Return aggregate statistics about the code graph (node/edge/file counts, DB size, etc.).".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {}
        }),
    }
}

fn def_files() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_files".to_string(),
        description: "List indexed project files. Use to explore file structure without reading file contents.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Filter to files under this directory path"
                },
                "pattern": {
                    "type": "string",
                    "description": "Filter files matching this glob pattern (e.g. '**/*.rs')"
                },
                "format": {
                    "type": "string",
                    "enum": ["flat", "grouped"],
                    "description": "Output format: flat (one per line) or grouped by directory (default: grouped)"
                }
            }
        }),
    }
}

fn def_affected() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_affected".to_string(),
        description: "Find test files affected by changed source files. BFS through file dependency graph to discover impacted tests.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "files": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of changed file paths to analyze"
                },
                "depth": {
                    "type": "number",
                    "description": "Maximum dependency traversal depth (default: 5)"
                },
                "filter": {
                    "type": "string",
                    "description": "Custom glob pattern for test files (default: common test patterns)"
                }
            },
            "required": ["files"]
        }),
    }
}

fn def_dead_code() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_dead_code".to_string(),
        description: "Find symbols with no incoming edges (potentially unreachable code). Excludes main, test functions, and public items.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "kinds": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Node kinds to check (default: [\"function\", \"method\"])"
                }
            }
        }),
    }
}

fn def_diff_context() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_diff_context".to_string(),
        description: "Given changed file paths, return semantic context: which symbols were modified, what depends on them, and affected tests.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "files": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of changed file paths"
                },
                "depth": {
                    "type": "number",
                    "description": "Maximum impact traversal depth (default: 2)"
                }
            },
            "required": ["files"]
        }),
    }
}

fn def_module_api() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_module_api".to_string(),
        description: "Show the public API surface of a file or directory: all pub symbols sorted by file and line.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path or directory prefix to inspect"
                }
            },
            "required": ["path"]
        }),
    }
}

fn def_circular() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_circular".to_string(),
        description: "Detect circular dependencies between files in the code graph.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "max_depth": {
                    "type": "number",
                    "description": "Maximum cycle detection depth (default: 10)"
                }
            }
        }),
    }
}

fn def_hotspots() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_hotspots".to_string(),
        description: "Find symbols with the highest connectivity (most incoming + outgoing edges).".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "number",
                    "description": "Maximum number of hotspots to return (default: 10)"
                }
            }
        }),
    }
}

fn def_similar() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_similar".to_string(),
        description: "Find symbols with similar names using full-text search and substring matching.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "symbol": {
                    "type": "string",
                    "description": "Symbol name to find similar matches for"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of results (default: 10)"
                }
            },
            "required": ["symbol"]
        }),
    }
}

fn def_rename_preview() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_rename_preview".to_string(),
        description: "Show all references to a symbol — all edges where the node appears as source or target.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "node_id": {
                    "type": "string",
                    "description": "The unique node ID to find references for"
                }
            },
            "required": ["node_id"]
        }),
    }
}

fn def_unused_imports() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_unused_imports".to_string(),
        description: "Find import/use nodes that are never referenced by any other node.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {}
        }),
    }
}

fn def_rank() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_rank".to_string(),
        description: "Rank nodes by relationship count. Answer questions like 'most implemented interface', 'most extended class', 'most called function', 'class that implements the most interfaces', or 'function that calls the most others'.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "edge_kind": {
                    "type": "string",
                    "enum": ["implements", "extends", "calls", "uses", "contains", "annotates", "derives_macro"],
                    "description": "The relationship type to rank by (e.g. 'implements' to find most-implemented interfaces)"
                },
                "direction": {
                    "type": "string",
                    "enum": ["incoming", "outgoing"],
                    "description": "Edge direction: 'incoming' ranks targets (default, e.g. most-implemented interface), 'outgoing' ranks sources (e.g. class that implements the most interfaces)"
                },
                "node_kind": {
                    "type": "string",
                    "description": "Optional filter for node kind (e.g. 'interface', 'class', 'trait', 'function', 'method')"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of results to return (default: 10)"
                }
            },
            "required": ["edge_kind"]
        }),
    }
}

fn def_largest() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_largest".to_string(),
        description: "Rank nodes by size (line count). Find the largest classes, longest methods, biggest enums, etc.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "node_kind": {
                    "type": "string",
                    "description": "Filter by node kind (e.g. 'class', 'method', 'function', 'interface', 'enum', 'struct')"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of results to return (default: 10)"
                }
            }
        }),
    }
}

fn def_coupling() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_coupling".to_string(),
        description: "Rank files by coupling: how many other files they depend on (fan_out) or are depended on by (fan_in). Identifies highly-coupled modules.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "direction": {
                    "type": "string",
                    "enum": ["fan_in", "fan_out"],
                    "description": "fan_in: files depended on by the most others. fan_out: files that depend on the most others (default: fan_in)"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of results to return (default: 10)"
                }
            }
        }),
    }
}

fn def_inheritance_depth() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_inheritance_depth".to_string(),
        description: "Find the deepest class/interface inheritance hierarchies by walking extends chains. Identifies over-deep type hierarchies.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "number",
                    "description": "Maximum number of results to return (default: 10)"
                }
            }
        }),
    }
}

fn def_distribution() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_distribution".to_string(),
        description: "Show node kind distribution (classes, methods, fields, etc.) per file or directory. Useful for understanding code structure and composition.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory or file path prefix to filter (e.g. 'src/main/java/com/example'). Omit for entire codebase."
                },
                "summary": {
                    "type": "boolean",
                    "description": "If true, aggregate counts across all matching files instead of per-file breakdown (default: false)"
                }
            }
        }),
    }
}

fn def_recursion() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_recursion".to_string(),
        description: "Detect recursive and mutually-recursive call cycles in the call graph. Identifies violations of the 'no recursion' rule (NASA Power of 10 Rule 1).".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "number",
                    "description": "Maximum number of cycles to return (default: 10)"
                }
            }
        }),
    }
}

fn def_complexity() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_complexity".to_string(),
        description: "Rank functions/methods by composite complexity: line count + call fan-out (×3) + call fan-in. Identifies the most complex symbols that may need decomposition.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "node_kind": {
                    "type": "string",
                    "description": "Filter by node kind (default: function and method)"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of results to return (default: 10)"
                }
            }
        }),
    }
}

fn def_doc_coverage() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_doc_coverage".to_string(),
        description: "Find public symbols missing documentation (docstrings). Identifies gaps in API documentation for functions, methods, classes, interfaces, traits, structs, and enums.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory or file path prefix to filter (e.g. 'src/main'). Omit for entire codebase."
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of results to return (default: 50)"
                }
            }
        }),
    }
}

fn def_god_class() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_god_class".to_string(),
        description: "Find classes with the most members (methods + fields). Identifies 'god classes' with excessive responsibility that may need decomposition.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "number",
                    "description": "Maximum number of results to return (default: 10)"
                }
            }
        }),
    }
}

fn def_changelog() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_changelog".to_string(),
        description: "Generate a semantic diff/changelog between two git refs, categorizing symbols as added, removed, or modified.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "from_ref": {
                    "type": "string",
                    "description": "Starting git ref (commit, branch, tag)"
                },
                "to_ref": {
                    "type": "string",
                    "description": "Ending git ref (commit, branch, tag)"
                }
            },
            "required": ["from_ref", "to_ref"]
        }),
    }
}

fn def_port_status() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_port_status".to_string(),
        description: "Compare symbols between a source directory and a target directory to track porting progress. Matches by name (case-insensitive) and compatible kind (e.g. class↔struct, interface↔trait).".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "source_dir": {
                    "type": "string",
                    "description": "Path prefix for source code (e.g. 'src/python/')"
                },
                "target_dir": {
                    "type": "string",
                    "description": "Path prefix for target code (e.g. 'src/rust/')"
                },
                "kinds": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Node kinds to compare (default: [\"function\", \"method\", \"class\", \"struct\", \"interface\", \"trait\", \"enum\", \"module\"])"
                }
            },
            "required": ["source_dir", "target_dir"]
        }),
    }
}

fn def_port_order() -> ToolDefinition {
    ToolDefinition {
        name: "tokensave_port_order".to_string(),
        description: "Return symbols from a source directory in topological dependency order — port leaves first (symbols with no internal dependencies), then symbols that depend only on already-listed symbols.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "source_dir": {
                    "type": "string",
                    "description": "Path prefix for source code (e.g. 'src/python/')"
                },
                "kinds": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Node kinds to include (default: [\"function\", \"method\", \"class\", \"struct\", \"interface\", \"trait\", \"enum\", \"module\"])"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of symbols to return (default: 50)"
                }
            },
            "required": ["source_dir"]
        }),
    }
}
