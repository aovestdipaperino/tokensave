use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::PythonExtractor;
use tokensave::types::*;

#[test]
fn test_py_file_node_is_root() {
    let source = r#"
def hello():
    pass
"#;
    let extractor = PythonExtractor;
    let result = extractor.extract("test.py", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let files: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "test.py");
}

#[test]
fn test_py_function_declaration() {
    let source = r#"
def add(a, b):
    return a + b

def helper():
    pass
"#;
    let extractor = PythonExtractor;
    let result = extractor.extract("math.py", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 2);
    let add_fn = fns.iter().find(|f| f.name == "add").unwrap();
    assert_eq!(add_fn.visibility, Visibility::Pub);
    assert!(add_fn.signature.as_ref().unwrap().contains("add"));
    assert!(add_fn.signature.as_ref().unwrap().contains("a, b"));
    let helper_fn = fns.iter().find(|f| f.name == "helper").unwrap();
    assert_eq!(helper_fn.visibility, Visibility::Pub);
}

#[test]
fn test_py_async_function() {
    let source = r#"
async def fetch_data(url):
    pass
"#;
    let extractor = PythonExtractor;
    let result = extractor.extract("async_mod.py", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].name, "fetch_data");
    assert!(
        fns[0].is_async,
        "async function should have is_async = true"
    );
}

#[test]
fn test_py_class_extraction() {
    let source = r#"
class MyClass:
    pass
"#;
    let extractor = PythonExtractor;
    let result = extractor.extract("classes.py", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let classes: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Class)
        .collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "MyClass");
    assert_eq!(classes[0].visibility, Visibility::Pub);
}

#[test]
fn test_py_method_extraction() {
    let source = r#"
class Dog:
    def bark(self):
        print("Woof!")

    def fetch(self, item):
        return item
"#;
    let extractor = PythonExtractor;
    let result = extractor.extract("dog.py", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 2);
    let bark = methods.iter().find(|m| m.name == "bark").unwrap();
    assert_eq!(bark.visibility, Visibility::Pub);
    let fetch = methods.iter().find(|m| m.name == "fetch").unwrap();
    assert_eq!(fetch.visibility, Visibility::Pub);
}

#[test]
fn test_py_decorator_extraction() {
    let source = r#"
@staticmethod
def my_func():
    pass
"#;
    let extractor = PythonExtractor;
    let result = extractor.extract("decorators.py", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let decorators: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Decorator)
        .collect();
    assert_eq!(decorators.len(), 1);
    assert_eq!(decorators[0].name, "staticmethod");
    // Check Annotates edge from decorator to the function
    let annotates: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Annotates)
        .collect();
    assert_eq!(annotates.len(), 1);
}

#[test]
fn test_py_decorator_with_args() {
    let source = r#"
class MyClass:
    @property
    def name(self):
        return self._name

    @name.setter
    def name(self, value):
        self._name = value
"#;
    let extractor = PythonExtractor;
    let result = extractor.extract("props.py", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let decorators: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Decorator)
        .collect();
    assert!(
        decorators.len() >= 2,
        "should have at least 2 decorators, got {}",
        decorators.len()
    );
    let annotates: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Annotates)
        .collect();
    assert!(
        annotates.len() >= 2,
        "should have at least 2 Annotates edges"
    );
}

#[test]
fn test_py_import_statement() {
    let source = r#"
import os
import sys
"#;
    let extractor = PythonExtractor;
    let result = extractor.extract("imports.py", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let uses: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Use)
        .collect();
    assert_eq!(uses.len(), 2);
    assert!(uses.iter().any(|u| u.name == "os"));
    assert!(uses.iter().any(|u| u.name == "sys"));
}

#[test]
fn test_py_from_import_statement() {
    let source = r#"
from os.path import join, exists
from collections import defaultdict
"#;
    let extractor = PythonExtractor;
    let result = extractor.extract("imports.py", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let uses: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Use)
        .collect();
    // from os.path import join, exists → 2 Use nodes
    // from collections import defaultdict → 1 Use node
    assert_eq!(
        uses.len(),
        3,
        "uses: {:?}",
        uses.iter().map(|u| &u.name).collect::<Vec<_>>()
    );
}

#[test]
fn test_py_docstring_function() {
    let source = r#"
def greet(name):
    """Greet someone by name."""
    print(f"Hello, {name}!")
"#;
    let extractor = PythonExtractor;
    let result = extractor.extract("greet.py", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    let docstring = fns[0].docstring.as_ref().expect("should have docstring");
    assert!(
        docstring.contains("Greet someone by name"),
        "docstring: {}",
        docstring
    );
}

#[test]
fn test_py_docstring_class() {
    let source = r#"
class Calculator:
    """A simple calculator class."""

    def add(self, a, b):
        return a + b
"#;
    let extractor = PythonExtractor;
    let result = extractor.extract("calc.py", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let classes: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Class)
        .collect();
    assert_eq!(classes.len(), 1);
    let docstring = classes[0]
        .docstring
        .as_ref()
        .expect("class should have docstring");
    assert!(
        docstring.contains("simple calculator"),
        "docstring: {}",
        docstring
    );
}

#[test]
fn test_py_docstring_triple_single_quotes() {
    let source = r#"
def process():
    '''Process data using triple single quotes.'''
    pass
"#;
    let extractor = PythonExtractor;
    let result = extractor.extract("proc.py", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    let docstring = fns[0].docstring.as_ref().expect("should have docstring");
    assert!(
        docstring.contains("Process data"),
        "docstring: {}",
        docstring
    );
}

#[test]
fn test_py_visibility_private_underscore() {
    let source = r#"
def _private_func():
    pass

def public_func():
    pass
"#;
    let extractor = PythonExtractor;
    let result = extractor.extract("vis.py", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    let private_fn = fns.iter().find(|f| f.name == "_private_func").unwrap();
    assert_eq!(private_fn.visibility, Visibility::Private);
    let public_fn = fns.iter().find(|f| f.name == "public_func").unwrap();
    assert_eq!(public_fn.visibility, Visibility::Pub);
}

#[test]
fn test_py_visibility_dunder() {
    let source = r#"
class MyClass:
    def __init__(self):
        pass

    def __mangled(self):
        pass

    def normal(self):
        pass
"#;
    let extractor = PythonExtractor;
    let result = extractor.extract("vis2.py", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    let init = methods.iter().find(|m| m.name == "__init__").unwrap();
    assert_eq!(
        init.visibility,
        Visibility::Pub,
        "__init__ should be Pub (dunder)"
    );
    let mangled = methods.iter().find(|m| m.name == "__mangled").unwrap();
    assert_eq!(
        mangled.visibility,
        Visibility::Private,
        "__mangled should be Private (name mangling)"
    );
    let normal = methods.iter().find(|m| m.name == "normal").unwrap();
    assert_eq!(normal.visibility, Visibility::Pub);
}

#[test]
fn test_py_module_level_constants() {
    let source = r#"
MAX_SIZE = 1024
MIN_VALUE = 0
some_var = "hello"
"#;
    let extractor = PythonExtractor;
    let result = extractor.extract("consts.py", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let consts: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Const)
        .collect();
    assert_eq!(
        consts.len(),
        2,
        "should detect UPPER_CASE assignments as consts: {:?}",
        consts.iter().map(|c| &c.name).collect::<Vec<_>>()
    );
    assert!(consts.iter().any(|c| c.name == "MAX_SIZE"));
    assert!(consts.iter().any(|c| c.name == "MIN_VALUE"));
}

#[test]
fn test_py_call_site_tracking() {
    let source = r#"
def main():
    print("hello")
    some_func(42)
"#;
    let extractor = PythonExtractor;
    let result = extractor.extract("main.py", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let call_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(
        call_refs.len() >= 2,
        "should have call refs for print and some_func, got: {:?}",
        call_refs
            .iter()
            .map(|r| &r.reference_name)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_py_nested_class() {
    let source = r#"
class Outer:
    class Inner:
        def method(self):
            pass
"#;
    let extractor = PythonExtractor;
    let result = extractor.extract("nested.py", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let classes: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Class)
        .collect();
    assert_eq!(classes.len(), 2);
    assert!(classes.iter().any(|c| c.name == "Outer"));
    assert!(classes.iter().any(|c| c.name == "Inner"));
}

#[test]
fn test_py_contains_edges() {
    let source = r#"
class Dog:
    def bark(self):
        pass

def standalone():
    pass
"#;
    let extractor = PythonExtractor;
    let result = extractor.extract("edges.py", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    // File → Class, File → Function, Class → Method
    assert!(
        contains.len() >= 3,
        "should have at least 3 Contains edges, got {}",
        contains.len()
    );
}

#[test]
fn test_py_class_inheritance() {
    let source = r#"
class Animal:
    pass

class Dog(Animal):
    pass
"#;
    let extractor = PythonExtractor;
    let result = extractor.extract("inherit.py", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let has_extends = result.edges.iter().any(|e| e.kind == EdgeKind::Extends)
        || result
            .unresolved_refs
            .iter()
            .any(|r| r.reference_kind == EdgeKind::Extends);
    assert!(has_extends, "should detect class inheritance as Extends");
    // Check the reference name
    let extends_ref = result
        .unresolved_refs
        .iter()
        .find(|r| r.reference_kind == EdgeKind::Extends);
    if let Some(r) = extends_ref {
        assert_eq!(r.reference_name, "Animal");
    }
}

#[test]
fn test_py_class_multiple_inheritance() {
    let source = r#"
class Mixin:
    pass

class Base:
    pass

class Child(Base, Mixin):
    pass
"#;
    let extractor = PythonExtractor;
    let result = extractor.extract("multi.py", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let extends_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Extends)
        .collect();
    assert!(
        extends_refs.len() >= 2,
        "should have Extends refs for Base and Mixin, got: {:?}",
        extends_refs
            .iter()
            .map(|r| &r.reference_name)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_py_qualified_names() {
    let source = r#"
class MyClass:
    def method(self):
        pass
"#;
    let extractor = PythonExtractor;
    let result = extractor.extract("pkg/module.py", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 1);
    assert!(
        methods[0].qualified_name.contains("module.py"),
        "qualified_name should contain file path: {}",
        methods[0].qualified_name
    );
    assert!(
        methods[0].qualified_name.contains("MyClass"),
        "qualified_name should contain class name: {}",
        methods[0].qualified_name
    );
    assert!(
        methods[0].qualified_name.contains("method"),
        "qualified_name should contain method name: {}",
        methods[0].qualified_name
    );
}

#[test]
fn test_py_async_method() {
    let source = r#"
class Server:
    async def handle_request(self, request):
        pass
"#;
    let extractor = PythonExtractor;
    let result = extractor.extract("server.py", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 1);
    assert!(
        methods[0].is_async,
        "async method should have is_async = true"
    );
}

#[test]
fn test_py_extensions() {
    let extractor = PythonExtractor;
    assert_eq!(extractor.extensions(), &["py"]);
    assert_eq!(extractor.language_name(), "Python");
}

/// #141: receiver-typed method calls emit `Type::method`. Type is inferred
/// from `x = ClassName()` (CapWords) and `self`.
#[test]
fn test_receiver_typed_method_calls_python() {
    let source = "class Alpha:\n    def handle(self):\n        return 1\n    def run(self):\n        return self.handle()\n\nclass Beta:\n    def handle(self):\n        return 2\n\ndef main():\n    a = Alpha()\n    b = Beta()\n    return a.handle() + b.handle()\n";
    let result = PythonExtractor.extract("m.py", source);
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

/// #224: a call inside a closure nested in a factory function must still be
/// tracked as a `Calls` ref, and the closure itself must be indexed as a
/// `Function` node (not silently dropped, and not misclassified as a
/// `Method` just because it happens to sit inside a class's method below).
#[test]
fn test_py_nested_closure_call_tracked() {
    let source = r#"
def _helper(x: int) -> int:
    return x + 1

def make_adder():
    def add(x: int) -> int:
        return _helper(x)
    return add
"#;
    let result = PythonExtractor.extract("closure.py", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let functions: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert!(
        functions.iter().any(|f| f.name == "add"),
        "nested closure `add` should be indexed as a Function, got: {:?}",
        functions.iter().map(|f| &f.name).collect::<Vec<_>>()
    );

    let helper_calls: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls && r.reference_name == "_helper")
        .collect();
    assert!(
        !helper_calls.is_empty(),
        "closure's call to _helper should produce a Calls ref"
    );

    // Contains edge: make_adder -> add.
    let make_adder_id = result
        .nodes
        .iter()
        .find(|n| n.name == "make_adder")
        .map(|n| n.id.clone())
        .expect("make_adder node");
    let add_id = functions
        .iter()
        .find(|f| f.name == "add")
        .map(|f| f.id.clone())
        .expect("add node");
    assert!(
        result.edges.iter().any(|e| e.kind == EdgeKind::Contains
            && e.source == make_adder_id
            && e.target == add_id),
        "make_adder should Contains add"
    );

    // #224: `return add` returns the nested closure by name — without a
    // `Uses` ref here, `add` would itself look dead the moment Fix 3 started
    // indexing it as a node (it wasn't indexed at all before).
    let add_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Uses && r.reference_name == "add")
        .collect();
    assert!(
        !add_refs.is_empty(),
        "`return add` should produce a Uses ref for the returned closure"
    );
}

/// #224: a closure nested inside a *method* is a plain local function, not
/// a class member — `class_depth` must not leak into nested-def visitation.
#[test]
fn test_py_nested_closure_inside_method_is_function_not_method() {
    let source = r#"
class Factory:
    def make(self):
        def inner():
            return 1
        return inner
"#;
    let result = PythonExtractor.extract("closure_method.py", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let inner = result
        .nodes
        .iter()
        .find(|n| n.name == "inner")
        .expect("inner node");
    assert_eq!(
        inner.kind,
        NodeKind::Function,
        "closure nested in a method should be a Function, not a Method"
    );
}

/// #224: a function referenced only by name as a dict value
/// (`PARSERS = {"text": _parse_text}`) must produce a `Uses` ref, or the
/// referenced function looks dead even though it's reachable via the table.
#[test]
fn test_py_first_class_ref_in_dict_value() {
    let source = r#"
def _parse_text(s):
    return s

PARSERS = {"text": _parse_text}
"#;
    let result = PythonExtractor.extract("parsers.py", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let uses_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Uses && r.reference_name == "_parse_text")
        .collect();
    assert!(
        !uses_refs.is_empty(),
        "dict value referencing _parse_text should produce a Uses ref"
    );

    // The dict key ("text") must NOT be treated as a reference.
    assert!(
        result
            .unresolved_refs
            .iter()
            .all(|r| r.reference_name != "text"),
        "dict key must not be scanned as a value reference"
    );
}

/// #224: a function passed as a keyword argument value
/// (`QuestionSpec(parse=_parse_text)`) must produce a `Uses` ref; the
/// keyword name (`parse`) and the callee (`QuestionSpec`) must not.
#[test]
fn test_py_first_class_ref_in_keyword_argument() {
    let source = r#"
def _parse_text(s):
    return s

def build():
    return QuestionSpec(parse=_parse_text)
"#;
    let result = PythonExtractor.extract("spec.py", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let uses_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Uses && r.reference_name == "_parse_text")
        .collect();
    assert!(
        !uses_refs.is_empty(),
        "keyword argument value referencing _parse_text should produce a Uses ref"
    );
    assert!(
        result
            .unresolved_refs
            .iter()
            .all(|r| r.reference_name != "parse"),
        "keyword argument name must not be scanned as a value reference"
    );
}

/// #224 (second review): a function referenced only as a parameter default
/// (`def invoke(callback=_default_callback)`) must produce a `Uses` ref, or
/// the default looks dead even though it's the function's fallback value.
/// The default lives in the sibling `parameters` node, not the `block` body,
/// so it needs its own scan.
#[test]
fn test_py_first_class_ref_in_parameter_default() {
    let source = r#"
def _default_callback(x):
    return x

def invoke(callback=_default_callback):
    return callback()
"#;
    let result = PythonExtractor.extract("invoke.py", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let uses_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Uses && r.reference_name == "_default_callback")
        .collect();
    assert!(
        !uses_refs.is_empty(),
        "parameter default referencing _default_callback should produce a Uses ref"
    );

    // The parameter name itself must not be treated as a *value* reference
    // (a `Calls` ref for `callback()` in the body is separate, legitimate
    // call-site tracking — not what this scan produces).
    assert!(
        result
            .unresolved_refs
            .iter()
            .all(|r| !(r.reference_kind == EdgeKind::Uses && r.reference_name == "callback")),
        "parameter name must not be scanned as a value reference"
    );
}

/// #224 (second review): a function referenced only as a class-level
/// attribute value (`class Registry: CALLBACKS = {"x": _class_callback}`)
/// must produce a `Uses` ref. Previously the `expression_statement` dispatch
/// in `visit_node` was gated to module scope only, so a class-body
/// assignment never even reached the value-ref scanner.
#[test]
fn test_py_first_class_ref_in_class_level_assignment() {
    let source = r#"
def _class_callback(x):
    return x

class Registry:
    CALLBACKS = {"x": _class_callback}
"#;
    let result = PythonExtractor.extract("registry.py", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let uses_refs: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Uses && r.reference_name == "_class_callback")
        .collect();
    assert!(
        !uses_refs.is_empty(),
        "class-level assignment referencing _class_callback should produce a Uses ref"
    );

    // A class-body UPPER_SNAKE_CASE assignment is a class attribute, not a
    // module constant — it must not be indexed as a Const node (that would
    // change existing node counts for patterns like `Base.CLASS_VERSION`).
    assert!(
        result
            .nodes
            .iter()
            .all(|n| !(n.kind == NodeKind::Const && n.name == "CALLBACKS")),
        "class-level CALLBACKS must not become a module Const node"
    );
}
