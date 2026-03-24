use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::PascalExtractor;
use tokensave::types::*;

fn extract(source: &str) -> ExtractionResult {
    let extractor = PascalExtractor;
    extractor.extract("test.pas", source)
}

// ----------------------------
// File node
// ----------------------------

#[test]
fn test_pascal_file_node_is_root() {
    let result = extract("program Hello;\nbegin\nend.");
    let file_nodes: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::File).collect();
    assert_eq!(file_nodes.len(), 1);
    assert_eq!(file_nodes[0].name, "test.pas");
}

// ----------------------------
// Program declaration
// ----------------------------

#[test]
fn test_pascal_program_declaration() {
    let result = extract("program HelloWorld;\nbegin\nend.");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let progs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::PascalProgram)
        .collect();
    assert_eq!(progs.len(), 1);
    assert_eq!(progs[0].name, "HelloWorld");
    assert!(progs[0].signature.as_ref().unwrap().contains("program HelloWorld"));
}

#[test]
fn test_pascal_program_contains_edge() {
    let result = extract("program HelloWorld;\nbegin\nend.");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let file_id = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::File)
        .unwrap()
        .id
        .clone();
    let prog_id = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::PascalProgram)
        .unwrap()
        .id
        .clone();
    let contains = result
        .edges
        .iter()
        .any(|e| e.source == file_id && e.target == prog_id && e.kind == EdgeKind::Contains);
    assert!(contains, "File should contain PascalProgram");
}

// ----------------------------
// Unit declaration
// ----------------------------

#[test]
fn test_pascal_unit_declaration() {
    let result = extract("unit MyUnit;\n\ninterface\n\nimplementation\n\nend.");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let units: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::PascalUnit)
        .collect();
    assert_eq!(units.len(), 1);
    assert_eq!(units[0].name, "MyUnit");
    assert!(units[0].signature.as_ref().unwrap().contains("unit MyUnit"));
}

// ----------------------------
// Uses clause
// ----------------------------

#[test]
fn test_pascal_uses_clause() {
    let result = extract(
        "unit Test;\n\ninterface\n\nuses SysUtils, Classes;\n\nimplementation\n\nend.",
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let uses: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Use).collect();
    assert_eq!(uses.len(), 2);
    let names: Vec<_> = uses.iter().map(|n| n.name.as_str()).collect();
    assert!(names.contains(&"SysUtils"), "Should have SysUtils, got {:?}", names);
    assert!(names.contains(&"Classes"), "Should have Classes, got {:?}", names);
}

#[test]
fn test_pascal_uses_in_implementation() {
    let result = extract(
        "unit Test;\n\ninterface\n\nuses SysUtils;\n\nimplementation\n\nuses Math;\n\nend.",
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let uses: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Use).collect();
    assert_eq!(uses.len(), 2);
    let names: Vec<_> = uses.iter().map(|n| n.name.as_str()).collect();
    assert!(names.contains(&"SysUtils"));
    assert!(names.contains(&"Math"));
}

#[test]
fn test_pascal_uses_unresolved_refs() {
    let result = extract(
        "unit Test;\n\ninterface\n\nuses SysUtils;\n\nimplementation\n\nend.",
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let uses_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Uses)
        .collect();
    assert!(
        uses_refs.iter().any(|r| r.reference_name == "SysUtils"),
        "Should have unresolved Uses ref for SysUtils"
    );
}

// ----------------------------
// Function extraction
// ----------------------------

#[test]
fn test_pascal_function_extraction() {
    let result = extract(
        r#"program Test;

function Add(a, b: Integer): Integer;
begin
  Result := a + b;
end;

begin
end."#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].name, "Add");
    assert!(fns[0].signature.as_ref().unwrap().contains("function Add"));
}

// ----------------------------
// Procedure extraction
// ----------------------------

#[test]
fn test_pascal_procedure_extraction() {
    let result = extract(
        r#"program Test;

procedure PrintHello;
begin
  WriteLn('Hello');
end;

begin
end."#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let procs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Procedure)
        .collect();
    assert_eq!(procs.len(), 1);
    assert_eq!(procs[0].name, "PrintHello");
}

// ----------------------------
// Class type extraction
// ----------------------------

#[test]
fn test_pascal_class_extraction() {
    let result = extract(
        r#"unit Test;

interface

type
  TMyClass = class(TObject)
  end;

implementation

end."#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let classes: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Class).collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "TMyClass");
    assert!(
        classes[0].signature.as_ref().unwrap().contains("class"),
        "Signature should mention class"
    );
}

#[test]
fn test_pascal_class_extends() {
    let result = extract(
        r#"unit Test;

interface

type
  TMyClass = class(TObject)
  end;

implementation

end."#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let extends_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Extends)
        .collect();
    assert!(
        extends_refs.iter().any(|r| r.reference_name == "TObject"),
        "Should have Extends ref for TObject"
    );
}

// ----------------------------
// Record type extraction
// ----------------------------

#[test]
fn test_pascal_record_extraction() {
    let result = extract(
        r#"unit Test;

interface

type
  TPoint = record
    X: Integer;
    Y: Integer;
  end;

implementation

end."#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let records: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::PascalRecord)
        .collect();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].name, "TPoint");
}

#[test]
fn test_pascal_record_fields() {
    let result = extract(
        r#"unit Test;

interface

type
  TPoint = record
    X: Integer;
    Y: Integer;
  end;

implementation

end."#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fields: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Field).collect();
    assert_eq!(fields.len(), 2);
    let names: Vec<_> = fields.iter().map(|n| n.name.as_str()).collect();
    assert!(names.contains(&"X"));
    assert!(names.contains(&"Y"));
}

// ----------------------------
// Interface type extraction
// ----------------------------

#[test]
fn test_pascal_interface_type_extraction() {
    let result = extract(
        r#"unit Test;

interface

type
  IMyInterface = interface
    procedure DoSomething;
    function GetValue: Integer;
  end;

implementation

end."#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let intfs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Interface)
        .collect();
    assert_eq!(intfs.len(), 1);
    assert_eq!(intfs[0].name, "IMyInterface");
}

#[test]
fn test_pascal_interface_methods() {
    let result = extract(
        r#"unit Test;

interface

type
  IMyInterface = interface
    procedure DoSomething;
    function GetValue: Integer;
  end;

implementation

end."#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    // Interface methods should be extracted.
    let procs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Procedure)
        .collect();
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert!(
        procs.iter().any(|n| n.name == "DoSomething"),
        "Should have DoSomething procedure"
    );
    assert!(
        fns.iter().any(|n| n.name == "GetValue"),
        "Should have GetValue function"
    );
}

// ----------------------------
// Method extraction (inside class)
// ----------------------------

#[test]
fn test_pascal_class_method_declarations() {
    let result = extract(
        r#"unit Test;

interface

type
  TMyClass = class
  public
    procedure DoSomething;
    function GetName: string;
  end;

implementation

end."#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let procs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Procedure)
        .collect();
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert!(procs.iter().any(|n| n.name == "DoSomething"));
    assert!(fns.iter().any(|n| n.name == "GetName"));
}

// ----------------------------
// Constructor/Destructor extraction
// ----------------------------

#[test]
fn test_pascal_constructor_extraction() {
    let result = extract(
        r#"unit Test;

interface

type
  TMyClass = class
  public
    constructor Create;
  end;

implementation

constructor TMyClass.Create;
begin
end;

end."#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let ctors: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Constructor)
        .collect();
    assert!(
        ctors.len() >= 1,
        "Should have at least one constructor, got {}",
        ctors.len()
    );
    assert!(ctors.iter().any(|n| n.name == "Create"));
}

#[test]
fn test_pascal_destructor_extraction() {
    let result = extract(
        r#"unit Test;

interface

type
  TMyClass = class
  public
    destructor Destroy; override;
  end;

implementation

destructor TMyClass.Destroy;
begin
  inherited;
end;

end."#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method && n.name == "Destroy")
        .collect();
    assert!(
        methods.len() >= 1,
        "Should have at least one destructor-as-method, got {}",
        methods.len()
    );
}

// ----------------------------
// Type declaration extraction
// ----------------------------

#[test]
fn test_pascal_type_alias_extraction() {
    let result = extract(
        r#"unit Test;

interface

type
  TMyInteger = Integer;

implementation

end."#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let aliases: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::TypeAlias)
        .collect();
    assert_eq!(aliases.len(), 1);
    assert_eq!(aliases[0].name, "TMyInteger");
}

// ----------------------------
// Constant extraction
// ----------------------------

#[test]
fn test_pascal_const_extraction() {
    let result = extract(
        r#"unit Test;

interface

const
  MAX_VALUE = 100;
  PI_APPROX = 3.14;

implementation

end."#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let consts: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Const).collect();
    assert_eq!(consts.len(), 2);
    let names: Vec<_> = consts.iter().map(|n| n.name.as_str()).collect();
    assert!(names.contains(&"MAX_VALUE"));
    assert!(names.contains(&"PI_APPROX"));
}

// ----------------------------
// Variable declaration extraction
// ----------------------------

#[test]
fn test_pascal_var_extraction() {
    let result = extract(
        r#"unit Test;

interface

var
  GlobalVar: Integer;

implementation

end."#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let vars: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Static).collect();
    assert_eq!(vars.len(), 1);
    assert_eq!(vars[0].name, "GlobalVar");
}

// ----------------------------
// Property extraction
// ----------------------------

#[test]
fn test_pascal_property_extraction() {
    let result = extract(
        r#"unit Test;

interface

type
  TMyClass = class
  private
    FName: string;
  public
    property Name: string read FName write FName;
  end;

implementation

end."#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let props: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Property)
        .collect();
    assert_eq!(props.len(), 1);
    assert_eq!(props[0].name, "Name");
    assert_eq!(props[0].visibility, Visibility::Pub);
}

// ----------------------------
// Visibility sections
// ----------------------------

#[test]
fn test_pascal_visibility_public() {
    let result = extract(
        r#"unit Test;

interface

type
  TMyClass = class
  public
    procedure PubMethod;
  end;

implementation

end."#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let pub_method = result
        .nodes
        .iter()
        .find(|n| n.name == "PubMethod")
        .expect("Should find PubMethod");
    assert_eq!(pub_method.visibility, Visibility::Pub);
}

#[test]
fn test_pascal_visibility_private() {
    let result = extract(
        r#"unit Test;

interface

type
  TMyClass = class
  private
    FField: Integer;
  end;

implementation

end."#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let field = result
        .nodes
        .iter()
        .find(|n| n.name == "FField")
        .expect("Should find FField");
    assert_eq!(field.visibility, Visibility::Private);
}

#[test]
fn test_pascal_visibility_protected() {
    let result = extract(
        r#"unit Test;

interface

type
  TMyClass = class
  protected
    procedure ProtMethod;
  end;

implementation

end."#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let prot_method = result
        .nodes
        .iter()
        .find(|n| n.name == "ProtMethod")
        .expect("Should find ProtMethod");
    assert_eq!(prot_method.visibility, Visibility::PubSuper);
}

// ----------------------------
// Comment extraction (docstrings)
// ----------------------------

#[test]
fn test_pascal_brace_comment_docstring() {
    let result = extract(
        r#"program Test;

{ This is a brace comment }
function Add(a, b: Integer): Integer;
begin
  Result := a + b;
end;

begin
end."#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let func = result
        .nodes
        .iter()
        .find(|n| n.name == "Add" && n.kind == NodeKind::Function)
        .expect("Should find Add function");
    assert!(
        func.docstring.is_some(),
        "Add should have a docstring from brace comment"
    );
    assert!(func.docstring.as_ref().unwrap().contains("brace comment"));
}

#[test]
fn test_pascal_oldstyle_comment_docstring() {
    let result = extract(
        r#"program Test;

(* This is an old-style comment *)
function Add(a, b: Integer): Integer;
begin
  Result := a + b;
end;

begin
end."#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let func = result
        .nodes
        .iter()
        .find(|n| n.name == "Add" && n.kind == NodeKind::Function)
        .expect("Should find Add function");
    assert!(
        func.docstring.is_some(),
        "Add should have a docstring from old-style comment"
    );
    assert!(func.docstring.as_ref().unwrap().contains("old-style comment"));
}

#[test]
fn test_pascal_line_comment_docstring() {
    let result = extract(
        r#"program Test;

// This is a line comment
function Add(a, b: Integer): Integer;
begin
  Result := a + b;
end;

begin
end."#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let func = result
        .nodes
        .iter()
        .find(|n| n.name == "Add" && n.kind == NodeKind::Function)
        .expect("Should find Add function");
    assert!(
        func.docstring.is_some(),
        "Add should have a docstring from line comment"
    );
    assert!(func.docstring.as_ref().unwrap().contains("line comment"));
}

// ----------------------------
// Call site tracking
// ----------------------------

#[test]
fn test_pascal_call_site_tracking() {
    let result = extract(
        r#"program Test;

procedure DoSomething;
begin
  WriteLn('Hello');
  DoOtherThing;
end;

begin
end."#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let call_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    let call_names: Vec<_> = call_refs.iter().map(|r| r.reference_name.as_str()).collect();
    assert!(
        call_names.contains(&"WriteLn"),
        "Should track WriteLn call, got {:?}",
        call_names
    );
    assert!(
        call_names.contains(&"DoOtherThing"),
        "Should track DoOtherThing call, got {:?}",
        call_names
    );
}

// ----------------------------
// Contains edges
// ----------------------------

#[test]
fn test_pascal_contains_edges() {
    let result = extract(
        r#"unit Test;

interface

type
  TMyClass = class
  public
    procedure DoSomething;
  end;

implementation

end."#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let class_id = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Class)
        .expect("Should find class")
        .id
        .clone();
    let method = result
        .nodes
        .iter()
        .find(|n| n.name == "DoSomething")
        .expect("Should find DoSomething");
    let contains = result.edges.iter().any(|e| {
        e.source == class_id && e.target == method.id && e.kind == EdgeKind::Contains
    });
    assert!(
        contains,
        "Class should contain DoSomething via Contains edge"
    );
}

#[test]
fn test_pascal_record_contains_fields() {
    let result = extract(
        r#"unit Test;

interface

type
  TPoint = record
    X: Integer;
    Y: Integer;
  end;

implementation

end."#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let record_id = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::PascalRecord)
        .expect("Should find record")
        .id
        .clone();
    let field_x = result
        .nodes
        .iter()
        .find(|n| n.name == "X" && n.kind == NodeKind::Field)
        .expect("Should find field X");
    let contains = result
        .edges
        .iter()
        .any(|e| e.source == record_id && e.target == field_x.id && e.kind == EdgeKind::Contains);
    assert!(contains, "Record should contain field X via Contains edge");
}

// ----------------------------
// Implementation section method
// ----------------------------

#[test]
fn test_pascal_implementation_method() {
    let result = extract(
        r#"unit Test;

interface

type
  TMyClass = class
  public
    procedure DoSomething;
  end;

implementation

procedure TMyClass.DoSomething;
begin
  WriteLn('Hello');
end;

end."#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    // The implementation method should be extracted as a Method node.
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method && n.name == "DoSomething")
        .collect();
    assert!(
        methods.len() >= 1,
        "Should have at least one Method node for DoSomething"
    );
}

#[test]
fn test_pascal_implementation_method_receives_class() {
    let result = extract(
        r#"unit Test;

interface

type
  TMyClass = class
  public
    procedure DoSomething;
  end;

implementation

procedure TMyClass.DoSomething;
begin
end;

end."#,
    );
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let receives_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Receives)
        .collect();
    assert!(
        receives_refs.iter().any(|r| r.reference_name == "TMyClass"),
        "Method implementation should have Receives ref to TMyClass"
    );
}

// ----------------------------
// Comprehensive unit test
// ----------------------------

#[test]
fn test_pascal_comprehensive_unit() {
    let source = r#"unit MyUnit;

interface

uses SysUtils, Classes;

type
  TMyClass = class(TObject)
  private
    FName: string;
  public
    constructor Create(const AName: string);
    destructor Destroy; override;
    procedure DoSomething;
    function GetName: string;
    property Name: string read FName write FName;
  end;

  TMyRecord = record
    X: Integer;
    Y: Integer;
  end;

  TMyAlias = Integer;

const
  MAX_VALUE = 100;

var
  GlobalVar: Integer;

implementation

uses Math;

constructor TMyClass.Create(const AName: string);
begin
  inherited Create;
  FName := AName;
end;

destructor TMyClass.Destroy;
begin
  inherited;
end;

procedure TMyClass.DoSomething;
begin
  WriteLn(FName);
end;

function TMyClass.GetName: string;
begin
  Result := FName;
end;

end."#;

    let result = extract(source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    // Should have a file node.
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::File));

    // Should have a unit node.
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::PascalUnit && n.name == "MyUnit"));

    // Should have uses.
    let uses: Vec<_> = result.nodes.iter().filter(|n| n.kind == NodeKind::Use).collect();
    assert!(uses.len() >= 3, "Should have at least 3 uses (SysUtils, Classes, Math)");

    // Should have the class.
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Class && n.name == "TMyClass"));

    // Should have the record.
    assert!(result
        .nodes
        .iter()
        .any(|n| n.kind == NodeKind::PascalRecord && n.name == "TMyRecord"));

    // Should have the type alias.
    assert!(result
        .nodes
        .iter()
        .any(|n| n.kind == NodeKind::TypeAlias && n.name == "TMyAlias"));

    // Should have constants.
    assert!(result
        .nodes
        .iter()
        .any(|n| n.kind == NodeKind::Const && n.name == "MAX_VALUE"));

    // Should have variables.
    assert!(result
        .nodes
        .iter()
        .any(|n| n.kind == NodeKind::Static && n.name == "GlobalVar"));

    // Should have the property.
    assert!(result
        .nodes
        .iter()
        .any(|n| n.kind == NodeKind::Property && n.name == "Name"));

    // Should have fields.
    assert!(result
        .nodes
        .iter()
        .any(|n| n.kind == NodeKind::Field && n.name == "FName"));

    // Should have Contains edges.
    assert!(
        !result.edges.is_empty(),
        "Should have Contains edges"
    );
    assert!(
        result.edges.iter().any(|e| e.kind == EdgeKind::Contains),
        "Should have at least one Contains edge"
    );
}

// ----------------------------
// LanguageExtractor trait
// ----------------------------

#[test]
fn test_pascal_extractor_extensions() {
    let extractor = PascalExtractor;
    let exts = extractor.extensions();
    assert!(exts.contains(&"pas"));
    assert!(exts.contains(&"pp"));
    assert!(exts.contains(&"dpr"));
    assert!(exts.contains(&"lpr"));
}

#[test]
fn test_pascal_extractor_language_name() {
    let extractor = PascalExtractor;
    assert_eq!(extractor.language_name(), "Pascal");
}
