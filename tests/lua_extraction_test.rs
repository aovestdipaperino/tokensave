use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::LuaExtractor;
use tokensave::types::*;

#[test]
fn test_lua_extract_functions() {
    let source = std::fs::read_to_string("tests/fixtures/sample.lua").unwrap();
    let extractor = LuaExtractor;
    let result = extractor.extract("sample.lua", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    // Functions: log (local), Connection.new, Pool.new
    assert_eq!(
        fns.len(),
        3,
        "expected 3 functions, got {}: {:?}",
        fns.len(),
        fns.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
    assert!(fns.iter().any(|n| n.name == "log"));
    assert!(
        fns.iter().any(|n| n.name == "new"),
        "Connection.new or Pool.new not found"
    );
}

#[test]
fn test_lua_extract_methods() {
    let source = std::fs::read_to_string("tests/fixtures/sample.lua").unwrap();
    let extractor = LuaExtractor;
    let result = extractor.extract("sample.lua", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    // Methods: Connection:connect, Connection:disconnect, Connection:isConnected,
    //          Pool:acquire, Pool:release
    assert_eq!(
        methods.len(),
        5,
        "expected 5 methods, got {}: {:?}",
        methods.len(),
        methods.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
    assert!(methods.iter().any(|n| n.name == "connect"));
    assert!(methods.iter().any(|n| n.name == "disconnect"));
    assert!(methods.iter().any(|n| n.name == "isConnected"));
    assert!(methods.iter().any(|n| n.name == "acquire"));
    assert!(methods.iter().any(|n| n.name == "release"));
}

#[test]
fn test_lua_extract_consts() {
    let source = std::fs::read_to_string("tests/fixtures/sample.lua").unwrap();
    let extractor = LuaExtractor;
    let result = extractor.extract("sample.lua", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let consts: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Const)
        .collect();
    assert_eq!(
        consts.len(),
        2,
        "expected 2 consts, got {}: {:?}",
        consts.len(),
        consts.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
    assert!(consts.iter().any(|n| n.name == "MAX_RETRIES"));
    assert!(consts.iter().any(|n| n.name == "DEFAULT_PORT"));
}

#[test]
fn test_lua_extract_requires() {
    let source = std::fs::read_to_string("tests/fixtures/sample.lua").unwrap();
    let extractor = LuaExtractor;
    let result = extractor.extract("sample.lua", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let uses: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Use)
        .collect();
    assert_eq!(uses.len(), 2, "expected 2 Use nodes, got {}", uses.len());
    assert!(uses.iter().any(|n| n.name == "json"));
    assert!(uses.iter().any(|n| n.name == "socket"));
}

#[test]
fn test_lua_call_sites() {
    let source = std::fs::read_to_string("tests/fixtures/sample.lua").unwrap();
    let extractor = LuaExtractor;
    let result = extractor.extract("sample.lua", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let call_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(!call_refs.is_empty(), "should have call refs");

    // log function calls print and string.format
    assert!(
        call_refs.iter().any(|r| r.reference_name == "print"),
        "should find print call"
    );
    assert!(
        call_refs
            .iter()
            .any(|r| r.reference_name == "string.format"),
        "should find string.format call"
    );

    // Connection.new calls setmetatable
    assert!(
        call_refs.iter().any(|r| r.reference_name == "setmetatable"),
        "should find setmetatable call"
    );

    // Connection:connect calls log
    assert!(
        call_refs.iter().any(|r| r.reference_name == "log"),
        "should find log call"
    );

    // Pool:acquire calls Connection.new
    assert!(
        call_refs
            .iter()
            .any(|r| r.reference_name == "Connection.new"),
        "should find Connection.new call"
    );
    // Pool:acquire calls conn:connect
    assert!(
        call_refs.iter().any(|r| r.reference_name == "conn:connect"),
        "should find conn:connect call"
    );

    // Pool:release calls table.insert
    assert!(
        call_refs.iter().any(|r| r.reference_name == "table.insert"),
        "should find table.insert call"
    );
}

#[test]
fn test_lua_docstrings() {
    let source = std::fs::read_to_string("tests/fixtures/sample.lua").unwrap();
    let extractor = LuaExtractor;
    let result = extractor.extract("sample.lua", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let log_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "log")
        .expect("log function not found");
    assert!(log_fn.docstring.is_some(), "log should have docstring");
    let doc = log_fn.docstring.as_ref().unwrap();
    assert!(
        doc.contains("Logs a message"),
        "docstring should contain 'Logs a message', got: {}",
        doc
    );

    let connect_method = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Method && n.name == "connect")
        .expect("connect method not found");
    assert!(
        connect_method
            .docstring
            .as_ref()
            .unwrap()
            .contains("Connects to the remote host"),
        "docstring: {:?}",
        connect_method.docstring
    );

    // MAX_RETRIES should have docstring
    let max_retries = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Const && n.name == "MAX_RETRIES")
        .expect("MAX_RETRIES not found");
    assert!(
        max_retries
            .docstring
            .as_ref()
            .unwrap()
            .contains("Maximum number of retries"),
        "docstring: {:?}",
        max_retries.docstring
    );
}

#[test]
fn test_lua_file_node() {
    let source = std::fs::read_to_string("tests/fixtures/sample.lua").unwrap();
    let extractor = LuaExtractor;
    let result = extractor.extract("sample.lua", &source);
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "sample.lua");
}

#[test]
fn test_lua_contains_edges() {
    let source = std::fs::read_to_string("tests/fixtures/sample.lua").unwrap();
    let extractor = LuaExtractor;
    let result = extractor.extract("sample.lua", &source);
    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    // File contains: 2 Use + 2 Const + 3 Function + 5 Method = 12 Contains edges
    assert!(
        contains.len() >= 12,
        "should have >= 12 Contains edges, got {}",
        contains.len()
    );
}

#[test]
fn test_lua_local_function_is_private() {
    let source = std::fs::read_to_string("tests/fixtures/sample.lua").unwrap();
    let extractor = LuaExtractor;
    let result = extractor.extract("sample.lua", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let log_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "log")
        .expect("log function not found");
    assert_eq!(
        log_fn.visibility,
        Visibility::Private,
        "local function should be private"
    );
}

#[test]
fn test_lua_dot_function_qualified_name() {
    let source = std::fs::read_to_string("tests/fixtures/sample.lua").unwrap();
    let extractor = LuaExtractor;
    let result = extractor.extract("sample.lua", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    // Connection.new should have qualified name containing Connection
    let conn_new_fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function && n.name == "new")
        .collect();
    assert!(
        conn_new_fns
            .iter()
            .any(|n| n.qualified_name.contains("Connection")),
        "Connection.new should have Connection in qualified name, got: {:?}",
        conn_new_fns
            .iter()
            .map(|n| &n.qualified_name)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_lua_signatures() {
    let source = std::fs::read_to_string("tests/fixtures/sample.lua").unwrap();
    let extractor = LuaExtractor;
    let result = extractor.extract("sample.lua", &source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let log_fn = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function && n.name == "log")
        .expect("log function not found");
    let sig = log_fn.signature.as_ref().unwrap();
    assert!(
        sig.contains("function")
            && sig.contains("log")
            && sig.contains("level")
            && sig.contains("message"),
        "log signature should contain function name and params, got: {}",
        sig
    );

    let connect = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Method && n.name == "connect")
        .expect("connect not found");
    let sig = connect.signature.as_ref().unwrap();
    assert!(
        sig.contains("Connection:connect"),
        "connect signature should contain Connection:connect, got: {}",
        sig
    );
}
