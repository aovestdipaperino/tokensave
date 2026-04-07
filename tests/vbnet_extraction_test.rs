use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::VbNetExtractor;
use tokensave::types::*;

#[test]
fn test_vb_file_node_is_root() {
    let source = "Class Main\nEnd Class";
    let extractor = VbNetExtractor;
    let result = extractor.extract("src/Main.vb", source);
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "src/Main.vb");
}

#[test]
fn test_vb_imports() {
    let source = r#"
Imports System
Imports System.Collections.Generic
"#;
    let extractor = VbNetExtractor;
    let result = extractor.extract("test.vb", source);
    let uses: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Use)
        .collect();
    assert_eq!(uses.len(), 2);
    assert!(uses.iter().any(|u| u.name == "System"));
    assert!(uses.iter().any(|u| u.name == "System.Collections.Generic"));
}

#[test]
fn test_vb_class() {
    let source = r#"
Class MyClass
    Public Property Name As String
End Class
"#;
    let extractor = VbNetExtractor;
    let result = extractor.extract("test.vb", source);
    let classes: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Class)
        .collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "MyClass");
}

#[test]
fn test_vb_class_docstring() {
    let source = r#"
''' <summary>
''' A test class.
''' </summary>
Class MyClass
End Class
"#;
    let extractor = VbNetExtractor;
    let result = extractor.extract("test.vb", source);
    let class = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Class && n.name == "MyClass")
        .expect("MyClass not found");
    assert!(class.docstring.is_some(), "Expected docstring on MyClass");
    assert!(
        class.docstring.as_ref().unwrap().contains("test class"),
        "Docstring should contain 'test class', got: {:?}",
        class.docstring
    );
}

#[test]
fn test_vb_inheritance() {
    let source = r#"
Class Base
End Class

Class Child
    Inherits Base
End Class
"#;
    let extractor = VbNetExtractor;
    let result = extractor.extract("test.vb", source);

    let extends: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Extends)
        .collect();
    assert!(
        extends.iter().any(|r| r.reference_name == "Base"),
        "Expected Extends ref to Base, got: {:?}",
        extends
    );
}

#[test]
fn test_vb_implements() {
    let source = r#"
Interface IFoo
    Function Bar() As String
End Interface

Class MyClass
    Implements IFoo
End Class
"#;
    let extractor = VbNetExtractor;
    let result = extractor.extract("test.vb", source);

    let impls: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Implements)
        .collect();
    assert!(
        impls.iter().any(|r| r.reference_name == "IFoo"),
        "Expected Implements ref to IFoo, got: {:?}",
        impls
    );
}

#[test]
fn test_vb_interface() {
    let source = r#"
Interface ISerializable
    Function ToJson() As String
End Interface
"#;
    let extractor = VbNetExtractor;
    let result = extractor.extract("test.vb", source);
    let interfaces: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Interface)
        .collect();
    assert_eq!(interfaces.len(), 1);
    assert_eq!(interfaces[0].name, "ISerializable");
}

#[test]
fn test_vb_struct() {
    let source = r#"
Structure Point
    Public X As Double
    Public Y As Double
End Structure
"#;
    let extractor = VbNetExtractor;
    let result = extractor.extract("test.vb", source);
    let structs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Struct)
        .collect();
    assert_eq!(structs.len(), 1);
    assert_eq!(structs[0].name, "Point");
}

#[test]
fn test_vb_module() {
    let source = r#"
Module Helpers
    Sub LogMessage(msg As String)
        Console.WriteLine(msg)
    End Sub
End Module
"#;
    let extractor = VbNetExtractor;
    let result = extractor.extract("test.vb", source);
    let modules: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Module)
        .collect();
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].name, "Helpers");
}

#[test]
fn test_vb_enum_with_variants() {
    let source = r#"
Enum LogLevel
    Debug
    Info
    Warning
End Enum
"#;
    let extractor = VbNetExtractor;
    let result = extractor.extract("test.vb", source);

    let enums: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Enum)
        .collect();
    assert_eq!(enums.len(), 1);
    assert_eq!(enums[0].name, "LogLevel");

    let variants: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::EnumVariant)
        .collect();
    assert_eq!(variants.len(), 3);
    assert!(variants.iter().any(|v| v.name == "Debug"));
    assert!(variants.iter().any(|v| v.name == "Info"));
    assert!(variants.iter().any(|v| v.name == "Warning"));
}

#[test]
fn test_vb_methods() {
    let source = r#"
Class Foo
    Public Function GetValue() As Integer
        Return 42
    End Function

    Public Sub DoWork()
        Console.WriteLine("working")
    End Sub
End Class
"#;
    let extractor = VbNetExtractor;
    let result = extractor.extract("test.vb", source);

    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert!(methods.len() >= 2, "expected >= 2 methods, got {}", methods.len());
    assert!(methods.iter().any(|m| m.name == "GetValue"));
    assert!(methods.iter().any(|m| m.name == "DoWork"));
}

#[test]
fn test_vb_constructor() {
    let source = r#"
Class Foo
    Sub New(name As String)
        Console.WriteLine(name)
    End Sub
End Class
"#;
    let extractor = VbNetExtractor;
    let result = extractor.extract("test.vb", source);

    let ctors: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Constructor)
        .collect();
    assert_eq!(ctors.len(), 1);
    assert_eq!(ctors[0].name, "New");
}

#[test]
fn test_vb_properties() {
    let source = r#"
Class Foo
    Public Property Name As String
    Public ReadOnly Property Id As Integer
End Class
"#;
    let extractor = VbNetExtractor;
    let result = extractor.extract("test.vb", source);

    let props: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Property)
        .collect();
    assert!(props.len() >= 2, "expected >= 2 properties, got {}", props.len());
    assert!(props.iter().any(|p| p.name == "Name"));
    assert!(props.iter().any(|p| p.name == "Id"));
}

#[test]
fn test_vb_const() {
    let source = r#"
Const MaxConnections As Integer = 100
"#;
    let extractor = VbNetExtractor;
    let result = extractor.extract("test.vb", source);

    let consts: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Const)
        .collect();
    assert_eq!(consts.len(), 1);
    assert_eq!(consts[0].name, "MaxConnections");
}

#[test]
fn test_vb_contains_edges() {
    let source = r#"
Class Foo
    Public Sub DoWork()
    End Sub
End Class
"#;
    let extractor = VbNetExtractor;
    let result = extractor.extract("test.vb", source);

    // File contains class, class contains method
    assert!(
        result.edges.iter().any(|e| e.kind == EdgeKind::Contains),
        "Expected Contains edges"
    );
}

#[test]
fn test_vb_call_sites() {
    let source = r#"
Class Foo
    Public Sub DoWork()
        Console.WriteLine("hello")
    End Sub
End Class
"#;
    let extractor = VbNetExtractor;
    let result = extractor.extract("test.vb", source);

    let calls: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(!calls.is_empty(), "Expected call site refs");
}

#[test]
fn test_vb_method_visibility() {
    let source = r#"
Class Foo
    Public Sub PublicMethod()
    End Sub

    Private Sub PrivateMethod()
    End Sub
End Class
"#;
    let extractor = VbNetExtractor;
    let result = extractor.extract("test.vb", source);

    let pub_method = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Method && n.name == "PublicMethod");
    assert!(pub_method.is_some());
    assert_eq!(pub_method.unwrap().visibility, Visibility::Pub);

    let priv_method = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Method && n.name == "PrivateMethod");
    assert!(priv_method.is_some());
    assert_eq!(priv_method.unwrap().visibility, Visibility::Private);
}

#[test]
fn test_vb_fields() {
    let source = r#"
Class Foo
    Private _value As Integer
End Class
"#;
    let extractor = VbNetExtractor;
    let result = extractor.extract("test.vb", source);

    let fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Field)
        .collect();
    assert!(fields.iter().any(|f| f.name == "_value"), "Expected _value field, got: {:?}", fields.iter().map(|f| &f.name).collect::<Vec<_>>());
}

#[test]
fn test_vb_attributes_on_class_and_method() {
    let source = r#"
<Serializable>
<Obsolete("message")>
Class MyClass
    <TestMethod>
    Sub DoSomething()
    End Sub
End Class
"#;
    let extractor = VbNetExtractor;
    let result = extractor.extract("attr.vb", source);

    // Should have 3 AnnotationUsage nodes: Serializable, Obsolete, TestMethod
    let annots: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::AnnotationUsage)
        .collect();
    assert_eq!(annots.len(), 3, "expected 3 annotations, got: {:?}", annots.iter().map(|a| &a.name).collect::<Vec<_>>());
    assert!(annots.iter().any(|a| a.name == "Serializable"));
    assert!(annots.iter().any(|a| a.name == "Obsolete"));
    assert!(annots.iter().any(|a| a.name == "TestMethod"));

    // Should have Annotates edges.
    let annotates_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Annotates)
        .collect();
    assert_eq!(annotates_edges.len(), 3, "expected 3 Annotates edges");

    // Should have Annotates unresolved refs.
    let annot_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Annotates)
        .collect();
    assert_eq!(annot_refs.len(), 3, "expected 3 Annotates refs");
}
