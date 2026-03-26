use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::ObjcExtractor;
use tokensave::types::*;

#[test]
fn test_objc_extract_imports() {
    let source = r#"#import <Foundation/Foundation.h>
#import "Connection.h"
#include <stdio.h>
"#;
    let extractor = ObjcExtractor;
    let result = extractor.extract("sample.m", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let includes: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Include)
        .collect();
    assert_eq!(includes.len(), 3);
    assert!(includes.iter().any(|n| n.name == "Foundation/Foundation.h"));
    assert!(includes.iter().any(|n| n.name == "Connection.h"));
    assert!(includes.iter().any(|n| n.name == "stdio.h"));
}

#[test]
fn test_objc_extract_preprocessor_defines() {
    let source = r#"#define MAX_RETRIES 3
#define DEFAULT_PORT 8080
"#;
    let extractor = ObjcExtractor;
    let result = extractor.extract("sample.m", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let defs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::PreprocessorDef)
        .collect();
    assert_eq!(defs.len(), 2);
    assert!(defs.iter().any(|n| n.name == "MAX_RETRIES"));
    assert!(defs.iter().any(|n| n.name == "DEFAULT_PORT"));
}

#[test]
fn test_objc_extract_ns_enum() {
    let source = r#"typedef NS_ENUM(NSInteger, LogLevel) {
    LogLevelDebug,
    LogLevelInfo,
    LogLevelWarning,
    LogLevelError
};
"#;
    let extractor = ObjcExtractor;
    let result = extractor.extract("sample.m", source);
    // NS_ENUM may produce parse errors but we still extract useful data
    let enums: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Enum)
        .collect();
    assert_eq!(enums.len(), 1, "expected 1 enum, got {}", enums.len());
    assert_eq!(enums[0].name, "LogLevel");

    let variants: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::EnumVariant)
        .collect();
    assert_eq!(variants.len(), 4, "expected 4 enum variants, got {}", variants.len());
    assert!(variants.iter().any(|n| n.name == "LogLevelDebug"));
    assert!(variants.iter().any(|n| n.name == "LogLevelInfo"));
    assert!(variants.iter().any(|n| n.name == "LogLevelWarning"));
    assert!(variants.iter().any(|n| n.name == "LogLevelError"));

    // Enum should contain its variants via Contains edges
    let enum_id = &enums[0].id;
    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains && &e.source == enum_id)
        .collect();
    assert_eq!(contains.len(), 4, "expected 4 Contains edges from enum");
}

#[test]
fn test_objc_extract_protocol() {
    let source = r#"/// Protocol for serializable objects.
@protocol Serializable <NSObject>
- (NSDictionary *)toJson;
- (NSString *)toJsonString;
@end
"#;
    let extractor = ObjcExtractor;
    let result = extractor.extract("sample.m", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let protocols: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Interface)
        .collect();
    assert_eq!(protocols.len(), 1);
    assert_eq!(protocols[0].name, "Serializable");
    assert!(
        protocols[0].docstring.as_ref().unwrap().contains("serializable"),
        "docstring: {:?}",
        protocols[0].docstring
    );

    // Protocol methods
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 2);
    assert!(methods.iter().any(|n| n.name == "toJson"));
    assert!(methods.iter().any(|n| n.name == "toJsonString"));

    // Protocol inherits from NSObject
    let implements: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Implements)
        .collect();
    assert!(implements.iter().any(|r| r.reference_name == "NSObject"));
}

#[test]
fn test_objc_extract_class_interface() {
    let source = r#"/// Base class with shared functionality.
@interface Base : NSObject
@property (nonatomic, strong, readonly) NSString *name;
- (instancetype)initWithName:(NSString *)name;
- (NSString *)description;
@end
"#;
    let extractor = ObjcExtractor;
    let result = extractor.extract("sample.m", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    // Class
    let classes: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Class)
        .collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "Base");
    assert!(
        classes[0].docstring.as_ref().unwrap().contains("Base class"),
        "docstring: {:?}",
        classes[0].docstring
    );

    // Property
    let props: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Property)
        .collect();
    assert_eq!(props.len(), 1);
    assert_eq!(props[0].name, "name");

    // Extends NSObject
    let extends: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Extends)
        .collect();
    assert!(extends.iter().any(|r| r.reference_name == "NSObject"));

    // Method declarations
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 2);
    assert!(methods.iter().any(|n| n.name == "initWithName"));
    assert!(methods.iter().any(|n| n.name == "description"));
}

#[test]
fn test_objc_extract_class_with_protocol_conformance() {
    let source = r#"@interface Connection : Base <Serializable>
@property (nonatomic, assign) NSInteger port;
@property (nonatomic, assign, readonly) BOOL connected;
- (instancetype)initWithHost:(NSString *)host port:(NSInteger)port;
- (BOOL)connect;
- (void)disconnect;
+ (instancetype)connectionWithHost:(NSString *)host;
@end
"#;
    let extractor = ObjcExtractor;
    let result = extractor.extract("sample.m", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    // Class
    let classes: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Class)
        .collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "Connection");

    // Extends Base
    assert!(result.unresolved_refs.iter().any(|r| {
        r.reference_kind == EdgeKind::Extends && r.reference_name == "Base"
    }));

    // Implements Serializable
    assert!(result.unresolved_refs.iter().any(|r| {
        r.reference_kind == EdgeKind::Implements && r.reference_name == "Serializable"
    }));

    // Properties
    let props: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Property)
        .collect();
    assert_eq!(props.len(), 2);
    assert!(props.iter().any(|n| n.name == "port"));
    assert!(props.iter().any(|n| n.name == "connected"));

    // Methods (instance and class)
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 4, "expected 4 method declarations: {:?}", methods.iter().map(|m| &m.name).collect::<Vec<_>>());
    assert!(methods.iter().any(|n| n.name == "initWithHost"));
    assert!(methods.iter().any(|n| n.name == "connect"));
    assert!(methods.iter().any(|n| n.name == "disconnect"));
    assert!(methods.iter().any(|n| n.name == "connectionWithHost"));
}

#[test]
fn test_objc_extract_implementation() {
    let source = r#"@implementation Base

- (instancetype)initWithName:(NSString *)name {
    self = [super init];
    if (self) {
        _name = [name copy];
    }
    return self;
}

- (NSString *)description {
    return [NSString stringWithFormat:@"%@(%@)",
            NSStringFromClass([self class]), self.name];
}

/// Private validation helper.
- (void)validate {
    NSAssert(self.name.length > 0, @"Name must not be empty");
}

@end
"#;
    let extractor = ObjcExtractor;
    let result = extractor.extract("sample.m", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    // Impl block
    let impls: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Impl)
        .collect();
    assert_eq!(impls.len(), 1);
    assert_eq!(impls[0].name, "Base");

    // Methods inside implementation
    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 3, "expected 3 methods: {:?}", methods.iter().map(|m| &m.name).collect::<Vec<_>>());
    assert!(methods.iter().any(|n| n.name == "initWithName"));
    assert!(methods.iter().any(|n| n.name == "description"));
    assert!(methods.iter().any(|n| n.name == "validate"));

    // Docstring on validate
    let validate = methods.iter().find(|m| m.name == "validate").unwrap();
    assert!(
        validate.docstring.as_ref().unwrap().contains("Private validation"),
        "validate docstring: {:?}",
        validate.docstring
    );

    // Call sites from message expressions
    let calls: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(!calls.is_empty(), "expected call site refs");

    // Contains edges
    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    assert!(contains.len() >= 3, "expected >= 3 Contains edges");
}

#[test]
fn test_objc_extract_c_function() {
    let source = r#"/// Top-level C function for logging.
void logMessage(LogLevel level, NSString *message) {
    NSLog(@"[%ld] %@", (long)level, message);
}
"#;
    let extractor = ObjcExtractor;
    let result = extractor.extract("sample.m", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let fns: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function)
        .collect();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].name, "logMessage");
    assert!(
        fns[0].docstring.as_ref().unwrap().contains("Top-level C function"),
        "docstring: {:?}",
        fns[0].docstring
    );
    assert!(fns[0].signature.as_ref().unwrap().contains("logMessage"));

    // Call sites
    let calls: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(!calls.is_empty(), "expected call site refs from NSLog");
}

#[test]
fn test_objc_message_expression_calls() {
    let source = r#"@implementation Foo

- (void)bar {
    [self doSomething];
    [NSString stringWithFormat:@"test"];
    NSLog(@"hello");
}

@end
"#;
    let extractor = ObjcExtractor;
    let result = extractor.extract("sample.m", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let calls: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .collect();
    assert!(calls.len() >= 3, "expected >= 3 call refs, got {}", calls.len());
    // Message sends create receiver.method format
    assert!(calls.iter().any(|r| r.reference_name.contains("doSomething")));
    assert!(calls.iter().any(|r| r.reference_name.contains("stringWithFormat")));
    // NSLog is a C function call
    assert!(calls.iter().any(|r| r.reference_name == "NSLog"));
}

#[test]
fn test_objc_file_node() {
    let source = "#import <Foundation/Foundation.h>\n";
    let extractor = ObjcExtractor;
    let result = extractor.extract("sample.m", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::File));
}

#[test]
fn test_objc_contains_edges() {
    let source = r#"@interface Foo : NSObject
@property (nonatomic, strong) NSString *bar;
- (void)baz;
@end
"#;
    let extractor = ObjcExtractor;
    let result = extractor.extract("sample.m", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    // File -> Class, Class -> Property, Class -> Method
    assert!(contains.len() >= 3, "expected >= 3 Contains edges, got {}", contains.len());
}

#[test]
fn test_objc_class_method_vs_instance_method() {
    let source = r#"@interface Foo : NSObject
- (void)instanceMethod;
+ (void)classMethod;
@end
"#;
    let extractor = ObjcExtractor;
    let result = extractor.extract("sample.m", source);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    let methods: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Method)
        .collect();
    assert_eq!(methods.len(), 2);
    assert!(methods.iter().any(|n| n.name == "instanceMethod"));
    assert!(methods.iter().any(|n| n.name == "classMethod"));
}

#[test]
fn test_objc_full_sample_file() {
    let source = std::fs::read_to_string("tests/fixtures/sample.m")
        .expect("Failed to read sample.m");
    let extractor = ObjcExtractor;
    let result = extractor.extract("sample.m", &source);

    // File root
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::File));

    // Imports
    let includes: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Include)
        .collect();
    assert_eq!(includes.len(), 2, "expected 2 includes");

    // Preprocessor defines
    let defs: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::PreprocessorDef)
        .collect();
    assert_eq!(defs.len(), 2, "expected 2 preprocessor defs");

    // Enum
    let enums: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Enum)
        .collect();
    assert_eq!(enums.len(), 1);
    assert_eq!(enums[0].name, "LogLevel");

    // Enum variants
    let variants: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::EnumVariant)
        .collect();
    assert_eq!(variants.len(), 4);

    // Protocol
    let protocols: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Interface)
        .collect();
    assert_eq!(protocols.len(), 1);
    assert_eq!(protocols[0].name, "Serializable");

    // Classes
    let classes: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Class)
        .collect();
    assert_eq!(classes.len(), 2, "expected 2 classes (Base, Connection)");
    assert!(classes.iter().any(|n| n.name == "Base"));
    assert!(classes.iter().any(|n| n.name == "Connection"));

    // Implementations
    let impls: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Impl)
        .collect();
    assert_eq!(impls.len(), 2, "expected 2 implementations");

    // Properties
    let props: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Property)
        .collect();
    assert!(props.len() >= 3, "expected >= 3 properties");

    // C Function
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::Function && n.name == "logMessage"));

    // Extends refs
    assert!(result.unresolved_refs.iter().any(|r| {
        r.reference_kind == EdgeKind::Extends && r.reference_name == "NSObject"
    }));
    assert!(result.unresolved_refs.iter().any(|r| {
        r.reference_kind == EdgeKind::Extends && r.reference_name == "Base"
    }));

    // Implements refs
    assert!(result.unresolved_refs.iter().any(|r| {
        r.reference_kind == EdgeKind::Implements
    }));

    // Call sites
    assert!(result.unresolved_refs.iter().any(|r| r.reference_kind == EdgeKind::Calls));

    // Contains edges
    let contains: Vec<_> = result
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    assert!(contains.len() >= 15, "expected >= 15 Contains edges, got {}", contains.len());
}
