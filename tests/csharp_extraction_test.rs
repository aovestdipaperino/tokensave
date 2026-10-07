use tokensave::extraction::CSharpExtractor;
use tokensave::extraction::LanguageExtractor;
use tokensave::types::*;

#[test]
fn test_cs_file_node_is_root() {
    let source = "public class Main {}";
    let extractor = CSharpExtractor;
    let result = extractor.extract("src/Main.cs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "src/Main.cs");
}

#[test]
fn test_cs_namespace() {
    let source = r#"
namespace MyApp.Models
{
    public class User {}
}
"#;
    let extractor = CSharpExtractor;
    let result = extractor.extract("test.cs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let namespaces: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Namespace)
        .collect();
    assert_eq!(namespaces.len(), 1);
    assert_eq!(namespaces[0].name, "MyApp.Models");
}

#[test]
fn test_cs_using_directive() {
    let source = r#"
using System;
using System.Collections.Generic;
using System.Linq;

public class Foo {}
"#;
    let extractor = CSharpExtractor;
    let result = extractor.extract("test.cs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let uses: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Use)
        .collect();
    assert_eq!(uses.len(), 3);
    assert!(uses.iter().any(|u| u.name == "System"));
    assert!(uses.iter().any(|u| u.name == "System.Collections.Generic"));
}

#[test]
fn test_cs_class() {
    let source = r#"
namespace TestApp
{
    /// <summary>
    /// A simple calculator.
    /// </summary>
    public class Calculator
    {
        public int Add(int a, int b) { return a + b; }
    }
}
"#;
    let extractor = CSharpExtractor;
    let result = extractor.extract("Calculator.cs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let classes: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Class)
        .collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "Calculator");
    assert_eq!(classes[0].visibility, Visibility::Pub);
    assert!(
        classes[0]
            .docstring
            .as_ref()
            .unwrap()
            .contains("simple calculator"),
        "docstring: {:?}",
        classes[0].docstring
    );
}

#[test]
fn test_cs_struct() {
    let source = r#"
public struct Point
{
    public int X;
    public int Y;
}
"#;
    let extractor = CSharpExtractor;
    let result = extractor.extract("test.cs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let structs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Struct)
        .collect();
    assert_eq!(structs.len(), 1);
    assert_eq!(structs[0].name, "Point");
    assert_eq!(structs[0].visibility, Visibility::Pub);
}

#[test]
fn test_cs_interface() {
    let source = r#"
public interface IDrawable
{
    void Draw();
    double Area();
}
"#;
    let extractor = CSharpExtractor;
    let result = extractor.extract("test.cs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let ifaces: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Interface)
        .collect();
    assert_eq!(ifaces.len(), 1);
    assert_eq!(ifaces[0].name, "IDrawable");
}

#[test]
fn test_cs_enum_with_members() {
    let source = r#"
public enum Color
{
    Red,
    Green,
    Blue
}
"#;
    let extractor = CSharpExtractor;
    let result = extractor.extract("test.cs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let enums: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Enum)
        .collect();
    assert_eq!(enums.len(), 1);
    assert_eq!(enums[0].name, "Color");
    let variants: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::EnumVariant)
        .collect();
    assert_eq!(variants.len(), 3);
    assert!(variants.iter().any(|v| v.name == "Red"));
    assert!(variants.iter().any(|v| v.name == "Green"));
    assert!(variants.iter().any(|v| v.name == "Blue"));
}

#[test]
fn test_cs_method() {
    let source = r#"
public class Foo
{
    public void DoSomething() {}
    private int Compute(int x) { return x * 2; }
    protected string GetName() { return "foo"; }
}
"#;
    let extractor = CSharpExtractor;
    let result = extractor.extract("test.cs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 3);
    let do_something = methods.iter().find(|m| m.name == "DoSomething").unwrap();
    assert_eq!(do_something.visibility, Visibility::Pub);
    let compute = methods.iter().find(|m| m.name == "Compute").unwrap();
    assert_eq!(compute.visibility, Visibility::Private);
    let get_name = methods.iter().find(|m| m.name == "GetName").unwrap();
    assert_eq!(get_name.visibility, Visibility::PubSuper);
}

#[test]
fn test_cs_constructor() {
    let source = r#"
public class Person
{
    private string _name;
    public Person(string name)
    {
        _name = name;
    }
}
"#;
    let extractor = CSharpExtractor;
    let result = extractor.extract("test.cs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let constructors: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Constructor)
        .collect();
    assert_eq!(constructors.len(), 1);
    assert_eq!(constructors[0].name, "Person");
}

#[test]
fn test_cs_property() {
    let source = r#"
public class Config
{
    public string Name { get; set; }
    private int Port { get; set; }
}
"#;
    let extractor = CSharpExtractor;
    let result = extractor.extract("test.cs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let props: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::CSharpProperty)
        .collect();
    assert_eq!(props.len(), 2);
    let name_prop = props.iter().find(|p| p.name == "Name").unwrap();
    assert_eq!(name_prop.visibility, Visibility::Pub);
    let port_prop = props.iter().find(|p| p.name == "Port").unwrap();
    assert_eq!(port_prop.visibility, Visibility::Private);
}

#[test]
fn test_cs_field() {
    let source = r#"
public class Config
{
    public static readonly int MaxSize = 1024;
    private string _name;
    internal int _port;
}
"#;
    let extractor = CSharpExtractor;
    let result = extractor.extract("test.cs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Field)
        .collect();
    assert_eq!(
        fields.len(),
        3,
        "fields: {:?}",
        fields.iter().map(|f| &f.name).collect::<Vec<_>>()
    );
    let max_size = fields.iter().find(|f| f.name == "MaxSize").unwrap();
    assert_eq!(max_size.visibility, Visibility::Pub);
    let name_field = fields.iter().find(|f| f.name == "_name").unwrap();
    assert_eq!(name_field.visibility, Visibility::Private);
    let port_field = fields.iter().find(|f| f.name == "_port").unwrap();
    assert_eq!(port_field.visibility, Visibility::PubCrate);
}

#[test]
fn test_cs_record() {
    let source = r#"
public record Person(string Name, int Age);
"#;
    let extractor = CSharpExtractor;
    let result = extractor.extract("test.cs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let records: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Record)
        .collect();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].name, "Person");
    assert_eq!(records[0].visibility, Visibility::Pub);
}

#[test]
fn test_cs_delegate() {
    let source = r#"
public delegate void EventHandler(object sender, EventArgs e);
"#;
    let extractor = CSharpExtractor;
    let result = extractor.extract("test.cs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let delegates: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Delegate)
        .collect();
    assert_eq!(delegates.len(), 1);
    assert_eq!(delegates[0].name, "EventHandler");
    assert_eq!(delegates[0].visibility, Visibility::Pub);
}

#[test]
fn test_cs_event() {
    let source = r#"
public class Button
{
    public event EventHandler Click;
}
"#;
    let extractor = CSharpExtractor;
    let result = extractor.extract("test.cs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let events: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Event)
        .collect();
    assert_eq!(
        events.len(),
        1,
        "events: {:?}",
        result
            .nodes
            .iter()
            .map(|n| (&n.kind, &n.name))
            .collect::<Vec<_>>()
    );
    assert_eq!(events[0].name, "Click");
    assert_eq!(events[0].visibility, Visibility::Pub);
}

#[test]
fn test_cs_attribute() {
    let source = r#"
public class Foo
{
    [Obsolete("Use NewMethod instead")]
    public void OldMethod() {}

    [Serializable]
    public void NewMethod() {}
}
"#;
    let extractor = CSharpExtractor;
    let result = extractor.extract("test.cs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let annots: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::AnnotationUsage)
        .collect();
    assert!(
        annots.len() >= 2,
        "should extract attribute usages, got: {:?}",
        annots.iter().map(|a| &a.name).collect::<Vec<_>>()
    );
    let has_annotates = result.edges.iter().any(|e| e.kind == EdgeKind::Annotates)
        || result
            .unresolved_refs
            .iter()
            .any(|r| r.reference_kind == EdgeKind::Annotates);
    assert!(has_annotates, "should have Annotates edges");
}

#[test]
fn test_cs_inheritance() {
    let source = r#"
public interface IAnimal
{
    void Speak();
}

public class Animal
{
    public virtual void Eat() {}
}

public class Dog : Animal, IAnimal
{
    public void Speak() {}
}
"#;
    let extractor = CSharpExtractor;
    let result = extractor.extract("test.cs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let has_extends = result.edges.iter().any(|e| e.kind == EdgeKind::Extends)
        || result
            .unresolved_refs
            .iter()
            .any(|r| r.reference_kind == EdgeKind::Extends);
    assert!(has_extends, "should detect extends");
    let has_implements = result.edges.iter().any(|e| e.kind == EdgeKind::Implements)
        || result
            .unresolved_refs
            .iter()
            .any(|r| r.reference_kind == EdgeKind::Implements);
    assert!(has_implements, "should detect implements");
}

#[test]
fn test_cs_visibility() {
    let source = r#"
public class Foo
{
    public void PubMethod() {}
    private void PrivMethod() {}
    internal void InternalMethod() {}
    protected void ProtectedMethod() {}
}
"#;
    let extractor = CSharpExtractor;
    let result = extractor.extract("test.cs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    let pub_m = methods.iter().find(|m| m.name == "PubMethod").unwrap();
    assert_eq!(pub_m.visibility, Visibility::Pub);
    let priv_m = methods.iter().find(|m| m.name == "PrivMethod").unwrap();
    assert_eq!(priv_m.visibility, Visibility::Private);
    let internal_m = methods.iter().find(|m| m.name == "InternalMethod").unwrap();
    assert_eq!(internal_m.visibility, Visibility::PubCrate);
    let protected_m = methods
        .iter()
        .find(|m| m.name == "ProtectedMethod")
        .unwrap();
    assert_eq!(protected_m.visibility, Visibility::PubSuper);
}

#[test]
fn test_cs_xml_doc_comment() {
    let source = r#"
public class Foo
{
    /// <summary>
    /// Adds two numbers together.
    /// </summary>
    /// <param name="a">First number</param>
    /// <param name="b">Second number</param>
    /// <returns>The sum</returns>
    public int Add(int a, int b) { return a + b; }
}
"#;
    let extractor = CSharpExtractor;
    let result = extractor.extract("test.cs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 1);
    let docstring = methods[0].docstring.as_ref().unwrap();
    assert!(
        docstring.contains("Adds two numbers"),
        "docstring should contain cleaned text: {:?}",
        docstring
    );
    // Should not contain raw XML tags
    assert!(
        !docstring.contains("<summary>"),
        "docstring should not contain XML tags: {:?}",
        docstring
    );
}

#[test]
fn test_cs_call_sites() {
    let source = r#"
public class App
{
    public void Run()
    {
        Console.WriteLine("hello");
        Helper();
        var list = new List<string>();
    }
    private void Helper() {}
}
"#;
    let extractor = CSharpExtractor;
    let result = extractor.extract("test.cs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let call_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(
        !call_refs.is_empty(),
        "should have call refs, got: {:?}",
        result.unresolved_refs
    );
}

#[test]
fn test_cs_async_method() {
    let source = r#"
public class Service
{
    public async Task<string> FetchDataAsync()
    {
        return await Task.FromResult("data");
    }
    public void SyncMethod() {}
}
"#;
    let extractor = CSharpExtractor;
    let result = extractor.extract("test.cs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    let async_method = methods.iter().find(|m| m.name == "FetchDataAsync").unwrap();
    assert!(async_method.is_async, "FetchDataAsync should be async");
    let sync_method = methods.iter().find(|m| m.name == "SyncMethod").unwrap();
    assert!(!sync_method.is_async, "SyncMethod should not be async");
}

#[test]
fn test_cs_contains_edges() {
    let source = r#"
public class Foo
{
    private int _x;
    public string Name { get; set; }
    public void Bar() {}
}
"#;
    let extractor = CSharpExtractor;
    let result = extractor.extract("test.cs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    // File contains: Class; Class contains: Field, Property, Method
    assert!(
        contains.len() >= 4,
        "should have Contains edges: {}",
        contains.len()
    );
}

#[test]
fn test_cs_extensions() {
    let extractor = CSharpExtractor;
    assert_eq!(extractor.extensions(), &["cs"]);
    assert_eq!(extractor.language_name(), "C#");
}

#[test]
fn test_cs_qualified_names() {
    let source = r#"
namespace MyApp
{
    public class Service
    {
        public void Run() {}
    }
}
"#;
    let extractor = CSharpExtractor;
    let result = extractor.extract("src/Service.cs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 1);
    assert!(
        methods[0].qualified_name.contains("MyApp"),
        "qualified_name should contain namespace: {}",
        methods[0].qualified_name
    );
    assert!(
        methods[0].qualified_name.contains("Service"),
        "qualified_name should contain class: {}",
        methods[0].qualified_name
    );
    assert!(
        methods[0].qualified_name.contains("Run"),
        "qualified_name should contain method: {}",
        methods[0].qualified_name
    );
}

/// Calls refs recorded from the node(s) named `from` of kind `kind`.
fn call_refs_from(result: &ExtractionResult, kind: &NodeKind, from: &str) -> Vec<String> {
    let ids: Vec<&str> = result
        .nodes
        .iter()
        .filter(|n| n.kind == *kind && n.name == from)
        .map(|n| n.id.as_str())
        .collect();
    result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls && ids.contains(&r.from_node_id.as_str()))
        .map(|r| r.reference_name.clone())
        .collect()
}

/// #637: calls inside property accessors (block-bodied get/set/init,
/// expression-bodied accessors, expression-bodied properties, and property
/// initializers) are recorded with the property as the caller.
#[test]
fn test_cs_property_accessor_call_sites_637() {
    let source = r#"
namespace Demo {
    public class Holder {
        public int Id { get; set; }
        public string Label { get { return Helper.Format(Id); } }
        public string Short => Helper.Short(Id);
        public int Width { set { Helper.Store(value); } }
        public int Height { init { Helper.Init(value); } }
        public int Depth { get => Helper.Depth(); set => Helper.SetDepth(value); }
        public Holder Self { get; } = new Holder();
    }
}
"#;
    let result = CSharpExtractor.extract("Holder.cs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let p = &NodeKind::CSharpProperty;
    assert_eq!(call_refs_from(&result, p, "Label"), vec!["Helper.Format"]);
    assert_eq!(call_refs_from(&result, p, "Short"), vec!["Helper.Short"]);
    assert_eq!(call_refs_from(&result, p, "Width"), vec!["Helper.Store"]);
    assert_eq!(call_refs_from(&result, p, "Height"), vec!["Helper.Init"]);
    assert_eq!(
        call_refs_from(&result, p, "Depth"),
        vec!["Helper.Depth", "Helper.SetDepth"]
    );
    assert_eq!(call_refs_from(&result, p, "Self"), vec!["new Holder"]);
    assert!(call_refs_from(&result, p, "Id").is_empty());
}

/// #637: an attribute on a property is not part of its accessor bodies, so
/// `nameof(...)` inside it is not recorded as a call.
#[test]
fn test_cs_property_attribute_is_not_a_call_site_637() {
    let source = r#"
public class Holder {
    [Obsolete(nameof(Other))]
    public int Value { get { return Compute(); } }
}
"#;
    let result = CSharpExtractor.extract("Holder.cs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    assert_eq!(
        call_refs_from(&result, &NodeKind::CSharpProperty, "Value"),
        vec!["Compute"]
    );
}

/// #637: indexers become `CSharpProperty` nodes named `this`, and calls in
/// their accessors (or expression body) are recorded from that node.
#[test]
fn test_cs_indexer_call_sites_637() {
    let source = r#"
public class Bag {
    public int this[int i] { get { return Lookup(i); } set { Store(i, value); } }
    public string this[string k] => Find(k);
}
"#;
    let result = CSharpExtractor.extract("Bag.cs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let indexers: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::CSharpProperty && n.name == "this")
        .collect();
    assert_eq!(indexers.len(), 2, "indexers: {indexers:?}");
    assert!(
        indexers[0].qualified_name.ends_with("Bag::this"),
        "qualified_name: {}",
        indexers[0].qualified_name
    );
    assert_eq!(
        indexers[0].signature.as_deref(),
        Some("int this[int i]"),
        "signature"
    );
    assert!(
        result
            .edges
            .iter()
            .any(|e| e.kind == EdgeKind::Contains && e.target == indexers[0].id),
        "indexer must be contained by its class"
    );
    let mut calls = call_refs_from(&result, &NodeKind::CSharpProperty, "this");
    calls.sort();
    assert_eq!(calls, vec!["Find", "Lookup", "Store"]);
}

/// #637: calls in event `add`/`remove` accessors are recorded from the event.
#[test]
fn test_cs_event_accessor_call_sites_637() {
    let source = r#"
public class Button {
    public event EventHandler Click { add { Subscribe(value); } remove { Unsubscribe(value); } }
}
"#;
    let result = CSharpExtractor.extract("Button.cs", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    assert_eq!(
        call_refs_from(&result, &NodeKind::Event, "Click"),
        vec!["Subscribe", "Unsubscribe"]
    );
}

/// #637 end to end: the issue's repro produces `calls` edges from each
/// property to `Helper.Format`, just like the method control case.
#[tokio::test]
async fn test_cs_property_accessor_calls_resolve_637() {
    use tokensave::tokensave::TokenSave;

    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(
        root.join("Helper.cs"),
        "namespace Demo { public static class Helper { public static string Format(int id) { return id.ToString(); } } }\n",
    )
    .unwrap();
    std::fs::write(
        root.join("Holder.cs"),
        r#"namespace Demo {
    public class Holder {
        public int Id { get; set; }
        public string LabelViaMethod() { return Helper.Format(Id); }
        public string Label { get { return Helper.Format(Id); } }
        public string Short => Helper.Format(Id);
        public int Width { set { Helper.Format(value); } }
    }
}
"#,
    )
    .unwrap();

    let cg = TokenSave::init(root).await.unwrap();
    cg.sync().await.unwrap();

    let nodes = cg.db().get_all_nodes().await.unwrap();
    let edges = cg.db().get_all_edges().await.unwrap();
    let id_of = |name: &str| {
        nodes
            .iter()
            .find(|n| n.name == name)
            .map(|n| n.id.clone())
            .unwrap()
    };
    let format_id = id_of("Format");
    for caller in ["LabelViaMethod", "Label", "Short", "Width"] {
        let caller_id = id_of(caller);
        assert!(
            edges.iter().any(|e| e.kind == EdgeKind::Calls
                && e.source == caller_id
                && e.target == format_id),
            "expected a calls edge {caller} -> Helper.Format"
        );
    }
}
