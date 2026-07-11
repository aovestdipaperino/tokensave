use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::TypeScriptExtractor;
use tokensave::types::*;

#[test]
fn test_ts_file_node_is_root() {
    let source = r#"function main() {}"#;
    let extractor = TypeScriptExtractor;
    let result = extractor.extract("test.ts", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "test.ts");
    assert_eq!(files[0].visibility, Visibility::Pub);
}

#[test]
fn test_ts_function_declaration() {
    let source = r#"
function add(a: number, b: number): number {
    return a + b;
}

function helper(): void {}
"#;
    let extractor = TypeScriptExtractor;
    let result = extractor.extract("math.ts", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 2);
    let add_fn = fns.iter().find(|f| f.name == "add").unwrap();
    assert_eq!(add_fn.visibility, Visibility::Private); // not exported
    let helper_fn = fns.iter().find(|f| f.name == "helper").unwrap();
    assert_eq!(helper_fn.visibility, Visibility::Private);
}

#[test]
fn test_ts_exported_function_is_pub() {
    let source = r#"
export function greet(name: string): string {
    return "Hello, " + name;
}

function internal(): void {}
"#;
    let extractor = TypeScriptExtractor;
    let result = extractor.extract("greet.ts", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 2);
    let greet_fn = fns.iter().find(|f| f.name == "greet").unwrap();
    assert_eq!(greet_fn.visibility, Visibility::Pub);
    let internal_fn = fns.iter().find(|f| f.name == "internal").unwrap();
    assert_eq!(internal_fn.visibility, Visibility::Private);
}

#[test]
fn test_ts_arrow_function() {
    let source = r#"
const add = (a: number, b: number): number => a + b;

export const multiply = (a: number, b: number) => {
    return a * b;
};
"#;
    let extractor = TypeScriptExtractor;
    let result = extractor.extract("arrow.ts", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let arrows: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::ArrowFunction)
        .collect();
    assert_eq!(arrows.len(), 2);
    let add = arrows.iter().find(|f| f.name == "add").unwrap();
    assert_eq!(add.visibility, Visibility::Private);
    let multiply = arrows.iter().find(|f| f.name == "multiply").unwrap();
    assert_eq!(multiply.visibility, Visibility::Pub);
}

#[test]
fn test_ts_class_with_methods() {
    let source = r#"
export class MyClass {
    private name: string;
    public age: number;

    constructor(name: string) {
        this.name = name;
    }

    getName(): string {
        return this.name;
    }

    private helper(): void {}
}
"#;
    let extractor = TypeScriptExtractor;
    let result = extractor.extract("class.ts", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    // Check class
    let classes: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Class)
        .collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "MyClass");
    assert_eq!(classes[0].visibility, Visibility::Pub);

    // Check methods
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 2); // getName + helper
    let get_name = methods.iter().find(|m| m.name == "getName").unwrap();
    assert_eq!(get_name.visibility, Visibility::Pub); // no modifier = public
    let helper = methods.iter().find(|m| m.name == "helper").unwrap();
    assert_eq!(helper.visibility, Visibility::Private);

    // Check constructor
    let constructors: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Constructor)
        .collect();
    assert_eq!(constructors.len(), 1);

    // Check fields
    let fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Field)
        .collect();
    assert_eq!(fields.len(), 2);
    let name_field = fields.iter().find(|f| f.name == "name").unwrap();
    assert_eq!(name_field.visibility, Visibility::Private);
    let age_field = fields.iter().find(|f| f.name == "age").unwrap();
    assert_eq!(age_field.visibility, Visibility::Pub);

    // Check Contains edges: class -> methods/fields
    let class_id = &classes[0].id;
    let contains_edges: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.source == *class_id && e.kind == EdgeKind::Contains)
        .collect();
    // Should contain: constructor, getName, helper, name field, age field
    assert_eq!(contains_edges.len(), 5);
}

#[test]
fn test_ts_interface() {
    let source = r#"
export interface Printable {
    print(): void;
    toString(): string;
}
"#;
    let extractor = TypeScriptExtractor;
    let result = extractor.extract("iface.ts", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let interfaces: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Interface)
        .collect();
    assert_eq!(interfaces.len(), 1);
    assert_eq!(interfaces[0].name, "Printable");
    assert_eq!(interfaces[0].visibility, Visibility::Pub);

    // Check interface methods
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 2);
}

#[test]
fn test_ts_enum() {
    let source = r#"
export enum Color {
    Red,
    Green,
    Blue
}
"#;
    let extractor = TypeScriptExtractor;
    let result = extractor.extract("color.ts", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let enums: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Enum)
        .collect();
    assert_eq!(enums.len(), 1);
    assert_eq!(enums[0].name, "Color");
    assert_eq!(enums[0].visibility, Visibility::Pub);

    // Check enum variants
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
fn test_ts_import_export() {
    let source = r#"
import { foo, bar } from './utils';
import * as path from 'path';
"#;
    let extractor = TypeScriptExtractor;
    let result = extractor.extract("imports.ts", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let uses: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Use)
        .collect();
    assert_eq!(uses.len(), 2);
    assert!(uses.iter().any(|u| u.name == "./utils"));
    assert!(uses.iter().any(|u| u.name == "path"));

    // Check unresolved Uses references
    let use_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Uses)
        .collect();
    assert_eq!(use_refs.len(), 2);
}

#[test]
fn test_ts_async_function() {
    let source = r#"
export async function fetchData(url: string): Promise<string> {
    const response = await fetch(url);
    return response.text();
}

function syncHelper(): void {}
"#;
    let extractor = TypeScriptExtractor;
    let result = extractor.extract("async.ts", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    let fetch_fn = fns.iter().find(|f| f.name == "fetchData").unwrap();
    assert!(fetch_fn.is_async, "fetchData should be async");
    let sync_fn = fns.iter().find(|f| f.name == "syncHelper").unwrap();
    assert!(!sync_fn.is_async, "syncHelper should not be async");
}

#[test]
fn test_ts_decorator() {
    let source = r#"
@Injectable()
class Service {
    getData(): string { return "data"; }
}
"#;
    let extractor = TypeScriptExtractor;
    let result = extractor.extract("service.ts", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let decorators: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Decorator)
        .collect();
    assert_eq!(decorators.len(), 1);
    assert_eq!(decorators[0].name, "Injectable");

    // Check Annotates edge
    let annotates: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Annotates)
        .collect();
    assert_eq!(annotates.len(), 1);

    // The Annotates edge should point to the class
    let classes: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Class)
        .collect();
    assert_eq!(annotates[0].target, classes[0].id);
}

#[test]
fn test_ts_namespace() {
    let source = r#"
namespace MyNamespace {
    export function inner(): void {}
}
"#;
    let extractor = TypeScriptExtractor;
    let result = extractor.extract("ns.ts", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let namespaces: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Namespace)
        .collect();
    assert_eq!(namespaces.len(), 1);
    assert_eq!(namespaces[0].name, "MyNamespace");

    // Check that inner function is inside the namespace
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].name, "inner");
    assert_eq!(fns[0].visibility, Visibility::Pub); // exported from namespace
}

#[test]
fn test_ts_jsdoc_docstring() {
    let source = r#"
/** Adds two numbers together. */
function add(a: number, b: number): number {
    return a + b;
}
"#;
    let extractor = TypeScriptExtractor;
    let result = extractor.extract("doc.ts", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    let docstring = fns[0].docstring.as_ref().expect("should have docstring");
    assert!(
        docstring.contains("Adds two numbers together"),
        "docstring should contain the JSDoc text, got: {docstring}"
    );
}

#[test]
fn test_ts_jsdoc_on_exported_function() {
    let source = r#"
/** Greets someone by name. */
export function greet(name: string): string {
    return "Hello, " + name;
}
"#;
    let extractor = TypeScriptExtractor;
    let result = extractor.extract("doc_export.ts", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    let docstring = fns[0]
        .docstring
        .as_ref()
        .expect("exported function should have docstring");
    assert!(
        docstring.contains("Greets someone by name"),
        "docstring should contain the JSDoc text, got: {docstring}"
    );
}

#[test]
fn test_ts_call_site_tracking() {
    let source = r#"
function greet(name: string): void {
    console.log("Hello", name);
}

function main(): void {
    greet("world");
}
"#;
    let extractor = TypeScriptExtractor;
    let result = extractor.extract("calls.ts", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let call_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(!call_refs.is_empty(), "should have call refs");
    // Should have: console.log from greet, greet from main
    assert!(
        call_refs.iter().any(|r| r.reference_name.contains("greet")),
        "should have a call to greet"
    );
    assert!(
        call_refs
            .iter()
            .any(|r| r.reference_name.contains("console.log")),
        "should have a call to console.log"
    );
}

#[test]
fn test_ts_type_alias() {
    let source = r#"
export type StringOrNum = string | number;
type ID = string;
"#;
    let extractor = TypeScriptExtractor;
    let result = extractor.extract("types.ts", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let aliases: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::TypeAlias)
        .collect();
    assert_eq!(aliases.len(), 2);
    let son = aliases.iter().find(|a| a.name == "StringOrNum").unwrap();
    assert_eq!(son.visibility, Visibility::Pub);
    let id_type = aliases.iter().find(|a| a.name == "ID").unwrap();
    assert_eq!(id_type.visibility, Visibility::Private);
}

#[test]
fn test_ts_class_extends_implements() {
    let source = r#"
interface Printable {
    print(): void;
}

class Base {}

export class Child extends Base implements Printable {
    print(): void {}
}
"#;
    let extractor = TypeScriptExtractor;
    let result = extractor.extract("inherit.ts", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    // Check for Extends unresolved ref
    let extends_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Extends)
        .collect();
    assert!(!extends_refs.is_empty(), "should have Extends ref for Base");
    assert!(extends_refs.iter().any(|r| r.reference_name == "Base"));

    // Check for Implements unresolved ref
    let impl_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Implements)
        .collect();
    assert!(
        !impl_refs.is_empty(),
        "should have Implements ref for Printable"
    );
    assert!(impl_refs.iter().any(|r| r.reference_name == "Printable"));
}

#[test]
fn test_ts_contains_edges() {
    let source = r#"
function foo(): void {}
"#;
    let extractor = TypeScriptExtractor;
    let result = extractor.extract("edges.ts", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let file_node = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::File)
        .unwrap();
    let fn_node = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Function)
        .unwrap();

    let contains = result
        .edges
        .iter()
        .find(|e| e.source == file_node.id && e.target == fn_node.id);
    assert!(
        contains.is_some(),
        "File should contain the function via Contains edge"
    );
    assert_eq!(contains.unwrap().kind, EdgeKind::Contains);
}

#[test]
fn test_js_file_uses_js_grammar() {
    let source = r#"
/** Adds two numbers */
function add(a, b) {
    return a + b;
}

export default class Foo {
    bar() { return 1; }
}
"#;
    let extractor = TypeScriptExtractor;
    let result = extractor.extract("test.js", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].name, "add");

    let classes: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Class)
        .collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "Foo");
    assert_eq!(classes[0].visibility, Visibility::Pub); // exported

    // Methods
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].name, "bar");
}

#[test]
fn test_ts_jsx_file() {
    let source = r#"
import React from 'react';

export function App() {
    return <div>Hello</div>;
}
"#;
    let extractor = TypeScriptExtractor;
    let result = extractor.extract("app.jsx", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert!(
        fns.iter().any(|f| f.name == "App"),
        "should extract App function from JSX"
    );
}

#[test]
fn test_ts_tsx_file() {
    let source = r#"
import React from 'react';

interface Props {
    name: string;
}

export const Greeting: React.FC<Props> = ({ name }) => {
    return <div>Hello, {name}</div>;
};
"#;
    let extractor = TypeScriptExtractor;
    let result = extractor.extract("greeting.tsx", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let interfaces: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Interface)
        .collect();
    assert_eq!(interfaces.len(), 1);
    assert_eq!(interfaces[0].name, "Props");

    // Arrow function component
    let arrows: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::ArrowFunction)
        .collect();
    assert_eq!(arrows.len(), 1);
    assert_eq!(arrows[0].name, "Greeting");
    assert_eq!(arrows[0].visibility, Visibility::Pub);
}

#[test]
fn test_ts_const_declaration() {
    let source = r#"
export const MAX_SIZE = 1024;
const SECRET = "hidden";
"#;
    let extractor = TypeScriptExtractor;
    let result = extractor.extract("consts.ts", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let consts: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Const)
        .collect();
    assert_eq!(consts.len(), 2);
    let max = consts.iter().find(|c| c.name == "MAX_SIZE").unwrap();
    assert_eq!(max.visibility, Visibility::Pub);
    let secret = consts.iter().find(|c| c.name == "SECRET").unwrap();
    assert_eq!(secret.visibility, Visibility::Private);
}

#[test]
fn test_ts_async_arrow_function() {
    let source = r#"
const fetchData = async (url: string) => {
    return await fetch(url);
};
"#;
    let extractor = TypeScriptExtractor;
    let result = extractor.extract("async_arrow.ts", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let arrows: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::ArrowFunction)
        .collect();
    assert_eq!(arrows.len(), 1);
    assert_eq!(arrows[0].name, "fetchData");
    assert!(arrows[0].is_async, "fetchData arrow should be async");
}

#[test]
fn test_ts_multiple_decorators() {
    let source = r#"
@Component({
    selector: 'app-root'
})
@Injectable()
class AppComponent {}
"#;
    let extractor = TypeScriptExtractor;
    let result = extractor.extract("decorators.ts", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let decorators: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Decorator)
        .collect();
    assert_eq!(decorators.len(), 2);
    assert!(decorators.iter().any(|d| d.name == "Component"));
    assert!(decorators.iter().any(|d| d.name == "Injectable"));
}

#[test]
fn test_ts_enum_private() {
    let source = r#"
enum Direction {
    Up,
    Down,
    Left,
    Right
}
"#;
    let extractor = TypeScriptExtractor;
    let result = extractor.extract("dir.ts", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let enums: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Enum)
        .collect();
    assert_eq!(enums.len(), 1);
    assert_eq!(enums[0].visibility, Visibility::Private); // not exported
}

/// #141: a method call through a typed binding must emit `Type::method` so the
/// resolver can disambiguate between classes that share a method name.
#[test]
fn test_receiver_typed_method_calls_ts() {
    let source = r#"
class Alpha { handle(): number { return 1; } }
class Beta { handle(): number { return 2; } }
function run(): number {
  const a = new Alpha();
  const b: Beta = new Beta();
  return a.handle() + b.handle();
}
"#;
    let result = TypeScriptExtractor.extract("svc.ts", source);
    let names: Vec<&str> = result
        .unresolved_refs
        .iter()
        .map(|u| u.reference_name.as_str())
        .collect();
    assert!(
        names.contains(&"Alpha::handle"),
        "expected Alpha::handle, got {names:?}"
    );
    assert!(
        names.contains(&"Beta::handle"),
        "expected Beta::handle, got {names:?}"
    );
}

// --- Regression tests for issues #205, #206, #209, #210, #211 ---

fn calls_from(result: &ExtractionResult, from_name_contains: &str) -> Vec<String> {
    let ids: Vec<&str> = result
        .nodes
        .iter()
        .filter(|n| n.name.contains(from_name_contains))
        .map(|n| n.id.as_str())
        .collect();
    result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls && ids.contains(&r.from_node_id.as_str()))
        .map(|r| r.reference_name.clone())
        .collect()
}

#[test]
fn test_ts_signature_with_destructured_params_not_truncated() {
    // #205
    let source = r#"
export function StatCard({ label, value, icon, color, suffix = "" }: StatCardProps) {
    return null;
}
"#;
    let result = TypeScriptExtractor.extract("card.tsx", source);
    let f = result.nodes.iter().find(|n| n.name == "StatCard").unwrap();
    let sig = f.signature.as_deref().unwrap();
    assert!(sig.contains("StatCardProps"), "signature truncated: {sig}");
    assert!(sig.contains("suffix"), "signature truncated: {sig}");
}

#[test]
fn test_ts_calls_inside_nested_arrow_callbacks() {
    // #209
    let source = r#"
function helper() {}

function Host() {
    useMemo(() => helper(), []);
    return null;
}

const HostArrow = () => helper();
"#;
    let result = TypeScriptExtractor.extract("host.tsx", source);
    let host_calls = calls_from(&result, "Host");
    assert!(
        host_calls.iter().any(|c| c == "helper"),
        "Host calls: {host_calls:?}"
    );
    let arrow_calls = calls_from(&result, "HostArrow");
    assert!(
        arrow_calls.iter().any(|c| c == "helper"),
        "HostArrow calls: {arrow_calls:?}"
    );
}

#[test]
fn test_tsx_jsx_render_creates_call_ref() {
    // #210
    let source = r#"
function Child() { return null; }
function Parent() { return <Child />; }
const ParentExpr = () => <Child />;
function Wrapper() { return <div><Child prop={1}>x</Child></div>; }
"#;
    let result = TypeScriptExtractor.extract("parent.tsx", source);
    for host in ["Parent", "ParentExpr", "Wrapper"] {
        let calls = calls_from(&result, host);
        assert!(
            calls.iter().any(|c| c == "Child"),
            "{host} should reference Child, got {calls:?}"
        );
        // Lowercase intrinsic tags must not be referenced.
        assert!(
            !calls.iter().any(|c| c == "div"),
            "{host} refs div: {calls:?}"
        );
    }
}

#[test]
fn test_ts_vitest_describe_it_indexed() {
    // #211
    let source = r#"
import { describe, it, expect } from "vitest";
import { formatDate, parseDate } from "./date";

describe("date utils", () => {
    it("formats a date", () => {
        expect(formatDate(new Date())).toBe("x");
    });
    it("parses a date", () => {
        parseDate("2024-01-01");
    });
});
"#;
    let result = TypeScriptExtractor.extract("date.test.ts", source);
    let test_nodes: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert!(
        test_nodes
            .iter()
            .any(|n| n.name.contains("describe date utils")),
        "missing describe node: {:?}",
        test_nodes.iter().map(|n| &n.name).collect::<Vec<_>>()
    );
    assert!(test_nodes
        .iter()
        .any(|n| n.name.contains("it formats a date")));
    let fmt_calls = calls_from(&result, "it formats a date");
    assert!(fmt_calls.iter().any(|c| c == "formatDate"), "{fmt_calls:?}");
    let parse_calls = calls_from(&result, "it parses a date");
    assert!(
        parse_calls.iter().any(|c| c == "parseDate"),
        "{parse_calls:?}"
    );
}

#[test]
fn test_ts_decorators_on_class_methods_and_params() {
    // #206
    let source = r#"
@Controller("admin")
export class AdminController {
    @Get(":id")
    findOne(@Param("id") id: string) {
        return id;
    }

    @Post()
    create(@Body() dto: CreateDto) {
        return dto;
    }
}
"#;
    let result = TypeScriptExtractor.extract("admin.controller.ts", source);
    let decorators: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Decorator)
        .map(|n| n.name.clone())
        .collect();
    for want in ["Controller", "Get", "Post", "Param", "Body"] {
        assert!(
            decorators.contains(&want.to_string()),
            "missing @{want}: {decorators:?}"
        );
    }
    let annotate_edges = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Annotates)
        .count();
    assert!(
        annotate_edges >= 5,
        "expected >=5 Annotates edges, got {annotate_edges}"
    );
}
