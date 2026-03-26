use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::ZigExtractor;
use tokensave::types::*;

#[test]
fn test_zig_extract_imports() {
    let source = r#"const std = @import("std");
const mem = @import("std").mem;
"#;
    let extractor = ZigExtractor;
    let result = extractor.extract("sample.zig", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let uses: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Use)
        .collect();
    assert_eq!(uses.len(), 2, "expected 2 imports, got {}", uses.len());
    assert!(uses.iter().any(|n| n.name == "std"));
}

#[test]
fn test_zig_extract_struct() {
    let source = r#"/// A 2D point.
const Point = struct {
    x: f64,
    y: f64,

    /// Calculate distance to another point.
    pub fn distance(self: Point, other: Point) f64 {
        const dx = self.x - other.x;
        const dy = self.y - other.y;
        return @sqrt(dx * dx + dy * dy);
    }

    pub fn origin() Point {
        return .{ .x = 0, .y = 0 };
    }
};
"#;
    let extractor = ZigExtractor;
    let result = extractor.extract("point.zig", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let structs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Struct)
        .collect();
    assert_eq!(structs.len(), 1);
    assert_eq!(structs[0].name, "Point");
    assert!(
        structs[0].docstring.as_ref().unwrap().contains("2D point"),
        "docstring: {:?}",
        structs[0].docstring
    );

    let fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Field)
        .collect();
    assert_eq!(fields.len(), 2);
    assert!(fields.iter().any(|f| f.name == "x"));
    assert!(fields.iter().any(|f| f.name == "y"));

    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 2);
    assert!(methods.iter().any(|m| m.name == "distance"));
    assert!(methods.iter().any(|m| m.name == "origin"));
}

#[test]
fn test_zig_extract_enum() {
    let source = r#"/// Represents a log level.
const LogLevel = enum {
    debug,
    info,
    warning,
    err,
};
"#;
    let extractor = ZigExtractor;
    let result = extractor.extract("log.zig", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let enums: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Enum)
        .collect();
    assert_eq!(enums.len(), 1);
    assert_eq!(enums[0].name, "LogLevel");
    assert!(
        enums[0]
            .docstring
            .as_ref()
            .unwrap()
            .contains("log level"),
        "docstring: {:?}",
        enums[0].docstring
    );

    let variants: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::EnumVariant)
        .collect();
    assert_eq!(variants.len(), 4);
    assert!(variants.iter().any(|v| v.name == "debug"));
    assert!(variants.iter().any(|v| v.name == "info"));
    assert!(variants.iter().any(|v| v.name == "warning"));
    assert!(variants.iter().any(|v| v.name == "err"));
}

#[test]
fn test_zig_top_level_functions() {
    let source = r#"/// Logs a message at the given level.
pub fn log(level: u8, message: []const u8) void {
    _ = level;
}

/// Processes connections.
pub fn processConnections(connections: []u8) u32 {
    var count: u32 = 0;
    return count;
}
"#;
    let extractor = ZigExtractor;
    let result = extractor.extract("funcs.zig", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 2);
    assert!(fns.iter().any(|f| f.name == "log"));
    assert!(fns.iter().any(|f| f.name == "processConnections"));

    // Both should be pub
    for f in &fns {
        assert_eq!(f.visibility, Visibility::Pub, "{} should be pub", f.name);
    }

    // Docstrings
    let log_fn = fns.iter().find(|f| f.name == "log").unwrap();
    assert!(
        log_fn.docstring.as_ref().unwrap().contains("Logs a message"),
        "docstring: {:?}",
        log_fn.docstring
    );
}

#[test]
fn test_zig_function_vs_method() {
    let source = r#"pub fn topLevel() void {}

const Foo = struct {
    pub fn method(self: Foo) void {}
};
"#;
    let extractor = ZigExtractor;
    let result = extractor.extract("funcs.zig", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].name, "topLevel");

    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].name, "method");
}

#[test]
fn test_zig_const_extraction() {
    let source = r#"/// Maximum number of connections allowed.
const max_connections: u32 = 100;
"#;
    let extractor = ZigExtractor;
    let result = extractor.extract("const.zig", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let consts: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Const)
        .collect();
    assert_eq!(consts.len(), 1);
    assert_eq!(consts[0].name, "max_connections");
    assert!(
        consts[0]
            .docstring
            .as_ref()
            .unwrap()
            .contains("Maximum number"),
        "docstring: {:?}",
        consts[0].docstring
    );
}

#[test]
fn test_zig_visibility() {
    let source = r#"const Foo = struct {
    pub fn publicMethod() void {}
    fn privateMethod() void {}
};

pub fn publicFn() void {}
fn privateFn() void {}
"#;
    let extractor = ZigExtractor;
    let result = extractor.extract("vis.zig", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let public_method = result
        .nodes
        .iter()
        .find(|n| n.name == "publicMethod")
        .expect("publicMethod not found");
    assert_eq!(public_method.visibility, Visibility::Pub);

    let private_method = result
        .nodes
        .iter()
        .find(|n| n.name == "privateMethod")
        .expect("privateMethod not found");
    assert_eq!(private_method.visibility, Visibility::Private);

    let public_fn = result
        .nodes
        .iter()
        .find(|n| n.name == "publicFn")
        .expect("publicFn not found");
    assert_eq!(public_fn.visibility, Visibility::Pub);

    let private_fn = result
        .nodes
        .iter()
        .find(|n| n.name == "privateFn")
        .expect("privateFn not found");
    assert_eq!(private_fn.visibility, Visibility::Private);
}

#[test]
fn test_zig_test_declaration() {
    let source = r#"const std = @import("std");

test "point distance" {
    const x: u32 = 3;
    try std.testing.expectEqual(@as(u32, 3), x);
}
"#;
    let extractor = ZigExtractor;
    let result = extractor.extract("test.zig", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    // test declarations are mapped as Function nodes
    let test_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "point distance");
    assert!(test_fn.is_some(), "test 'point distance' not found");
    assert_eq!(
        test_fn.unwrap().visibility,
        Visibility::Private,
        "test should be private"
    );
}

#[test]
fn test_zig_call_sites() {
    let source = r#"const std = @import("std");

pub fn greet() void {
    std.debug.print("hello\n", .{});
}

pub fn main() void {
    greet();
}
"#;
    let extractor = ZigExtractor;
    let result = extractor.extract("main.zig", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let call_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(!call_refs.is_empty(), "should have call refs");
    assert!(
        call_refs
            .iter()
            .any(|r| r.reference_name.contains("print")),
        "should find print call, got: {:?}",
        call_refs.iter().map(|r| &r.reference_name).collect::<Vec<_>>()
    );
    assert!(
        call_refs.iter().any(|r| r.reference_name == "greet"),
        "should find greet call"
    );
}

#[test]
fn test_zig_docstrings() {
    let source = r#"/// Initializes the system.
/// This is important.
pub fn setup() void {}
"#;
    let extractor = ZigExtractor;
    let result = extractor.extract("doc.zig", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    let doc = fns[0].docstring.as_ref().unwrap();
    assert!(
        doc.contains("Initializes the system"),
        "docstring: {:?}",
        doc
    );
    assert!(doc.contains("This is important"), "docstring: {:?}", doc);
}

#[test]
fn test_zig_file_node_is_root() {
    let source = r#"pub fn main() void {}
"#;
    let extractor = ZigExtractor;
    let result = extractor.extract("main.zig", source);
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "main.zig");
}

#[test]
fn test_zig_contains_edges() {
    let source = r#"const Foo = struct {
    x: u32,

    pub fn bar(self: Foo) void {}
};
"#;
    let extractor = ZigExtractor;
    let result = extractor.extract("foo.zig", source);
    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    // File -> Struct, Struct -> Field, Struct -> Method = 3 minimum
    assert!(
        contains.len() >= 3,
        "should have >= 3 Contains edges, got {}",
        contains.len()
    );
}

#[test]
fn test_zig_struct_with_multiple_methods() {
    let source = r#"const Connection = struct {
    host: []const u8,
    port: u16,
    connected: bool,

    /// Creates a new connection.
    pub fn init(host: []const u8, port: u16) Connection {
        return .{
            .host = host,
            .port = port,
            .connected = false,
        };
    }

    /// Establishes the connection.
    pub fn connect(self: *Connection) !void {
        self.connected = true;
    }

    pub fn disconnect(self: *Connection) void {
        self.connected = false;
    }

    pub fn isConnected(self: Connection) bool {
        return self.connected;
    }
};
"#;
    let extractor = ZigExtractor;
    let result = extractor.extract("conn.zig", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let structs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Struct)
        .collect();
    assert_eq!(structs.len(), 1);
    assert_eq!(structs[0].name, "Connection");

    let fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Field)
        .collect();
    assert_eq!(fields.len(), 3);
    assert!(fields.iter().any(|f| f.name == "host"));
    assert!(fields.iter().any(|f| f.name == "port"));
    assert!(fields.iter().any(|f| f.name == "connected"));

    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 4);
    assert!(methods.iter().any(|m| m.name == "init"));
    assert!(methods.iter().any(|m| m.name == "connect"));
    assert!(methods.iter().any(|m| m.name == "disconnect"));
    assert!(methods.iter().any(|m| m.name == "isConnected"));

    // Check docstrings on methods
    let init_method = methods.iter().find(|m| m.name == "init").unwrap();
    assert!(
        init_method
            .docstring
            .as_ref()
            .unwrap()
            .contains("Creates a new connection"),
        "init docstring: {:?}",
        init_method.docstring
    );
    let connect_method = methods.iter().find(|m| m.name == "connect").unwrap();
    assert!(
        connect_method
            .docstring
            .as_ref()
            .unwrap()
            .contains("Establishes"),
        "connect docstring: {:?}",
        connect_method.docstring
    );
}
