//! C# typed receivers (#642): a call through a field, property, primary
//! constructor parameter or local resolves through the receiver's static
//! type, and a receiver-qualified call never falls back onto the caller's own
//! same-named method.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use tempfile::TempDir;
use tokensave::extraction::{CSharpExtractor, LanguageExtractor};
use tokensave::tokensave::TokenSave;
use tokensave::types::{EdgeKind, Node};

const COORDINATOR: &str = r#"namespace App;

public sealed class Coordinator(Refresher refresher)
{
    public Task<Outcome> RefreshAsync(Provider provider) => Task.FromResult(new Outcome());

    private async Task<Outcome> PollAndRefreshAsync(Provider provider)
    {
        var result = await refresher.RefreshAsync(provider, "digest");
        return result;
    }

    private Task<Outcome> ThroughUnknown(Provider provider)
    {
        return Somewhere.current.RefreshAsync(provider, "digest");
    }
}
"#;

const REFRESHER: &str = r#"namespace App;

public sealed class Refresher
{
    public Task<Outcome> RefreshAsync(Provider provider, string digest) => Task.FromResult(new Outcome());
}
"#;

const SERVICE: &str = r#"namespace App;

internal sealed class RefreshService(Coordinator coordinator) : BackgroundService
{
    protected override async Task ExecuteAsync(CancellationToken stoppingToken)
    {
        var outcome = await coordinator.RefreshAsync(provider, ct: stoppingToken);
    }
}
"#;

const WRITERS: &str = r#"namespace App.Writing;

public interface IWriter<T>
{
    Task<int> WriteAsync(Stream stream, T[] blocks);
}

public sealed class Writer<T> : IWriter<T>
{
    public Task<int> WriteAsync(Stream stream, T[] blocks) => Task.FromResult(0);
}

public interface IWriterFactory
{
    IWriter<T> CreateWriter<T>(Options options);
}

public sealed class OtherSink
{
    public Task<int> WriteAsync(Stream stream) => Task.FromResult(0);
}
"#;

const EXPORTER: &str = r#"namespace App.Export;

public sealed class Exporter(IWriterFactory writerFactory)
{
    public async Task ExportAsync<T>(Stream stream, T[] blocks, Options options)
    {
        using var writer = writerFactory.CreateWriter<T>(options);
        var result = await writer.WriteAsync(stream, blocks);
    }

    public Task<int> WriteAsync(Stream stream) => Task.FromResult(1);
}
"#;

const CONSUMER: &str = r#"namespace App.Consumers;

public class Consumer
{
    private readonly Refresher _refresher = new Refresher();
    private Coordinator Coord { get; }

    public async Task Run(object raw)
    {
        await _refresher.RefreshAsync(null, "field");
        await Coord.RefreshAsync(null);
        Refresher typed = Make();
        await typed.RefreshAsync(null, "explicit");
        var built = new Coordinator(null);
        await built.RefreshAsync(null);
        var cast = (Refresher)raw;
        await cast.RefreshAsync(null, "cast");
        await this.RefreshAsync();
    }

    private Refresher Make() => new Refresher();

    public Task RefreshAsync() => Task.CompletedTask;
}
"#;

const EXTENSIONS: &str = r#"namespace App.Hosting;

public static class AppServiceExtensions
{
    public static IServiceCollection AddApp(this IServiceCollection services) => services;
}

public static class Startup
{
    public static void Configure(IServiceCollection services)
    {
        services.AddApp();
    }
}
"#;

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, text).unwrap();
}

fn node<'a>(nodes: &'a [Node], file: &str, name: &str) -> &'a Node {
    nodes
        .iter()
        .find(|n| n.file_path == file && n.name == name)
        .unwrap_or_else(|| panic!("no node {file}::{name}"))
}

/// Names of the callers of `file::name`, with the call lines.
async fn callers(cg: &TokenSave, file: &str, name: &str) -> BTreeSet<(String, u32)> {
    let nodes = cg.get_all_nodes().await.unwrap();
    let target = node(&nodes, file, name).id.clone();
    cg.get_incoming_edges(&target)
        .await
        .unwrap()
        .into_iter()
        .filter(|e| e.kind == EdgeKind::Calls)
        .map(|e| {
            let src = nodes
                .iter()
                .find(|n| n.id == e.source)
                .map_or_else(|| e.source.clone(), |n| n.name.clone());
            (src, e.line.unwrap_or(0))
        })
        .collect()
}

fn caller_names(set: &BTreeSet<(String, u32)>) -> BTreeSet<String> {
    set.iter().map(|(n, _)| n.clone()).collect()
}

async fn index(files: &[(&str, &str)]) -> (TempDir, TokenSave) {
    let dir = TempDir::new().unwrap();
    for (rel, text) in files {
        write(dir.path(), rel, text);
    }
    let cg = TokenSave::init(dir.path()).await.unwrap();
    cg.index_all().await.unwrap();
    (dir, cg)
}

fn all_files() -> Vec<(&'static str, &'static str)> {
    vec![
        ("src/Coordinator.cs", COORDINATOR),
        ("src/Refresher.cs", REFRESHER),
        ("src/Services/RefreshService.cs", SERVICE),
        ("src/Writing/Writers.cs", WRITERS),
        ("src/Export/Exporter.cs", EXPORTER),
        ("src/Consumers/Consumer.cs", CONSUMER),
        ("src/Hosting/Extensions.cs", EXTENSIONS),
    ]
}

/// Shape 1 and 2 of #642: a primary-constructor parameter types the receiver.
#[tokio::test]
async fn primary_constructor_parameter_types_the_receiver() {
    let (_dir, cg) = index(&all_files()).await;

    let coord = caller_names(&callers(&cg, "src/Coordinator.cs", "RefreshAsync").await);
    assert!(
        coord.contains("ExecuteAsync"),
        "RefreshService.ExecuteAsync calls Coordinator.RefreshAsync, got {coord:?}"
    );
    assert!(
        !coord.contains("PollAndRefreshAsync"),
        "PollAndRefreshAsync calls Refresher.RefreshAsync, not its own class's, got {coord:?}"
    );
    assert!(
        !coord.contains("ThroughUnknown"),
        "a receiver-qualified call must not bind to the caller's own method, got {coord:?}"
    );

    let refresher = caller_names(&callers(&cg, "src/Refresher.cs", "RefreshAsync").await);
    assert!(
        refresher.contains("PollAndRefreshAsync"),
        "PollAndRefreshAsync calls Refresher.RefreshAsync, got {refresher:?}"
    );
}

/// Shape 3 of #642: a `var` local initialised from a factory method takes
/// the factory's declared return type.
#[tokio::test]
async fn var_local_from_factory_takes_the_return_type() {
    let (_dir, cg) = index(&all_files()).await;

    let iface = callers(&cg, "src/Writing/Writers.cs", "WriteAsync").await;
    let nodes = cg.get_all_nodes().await.unwrap();
    let iface_method = nodes
        .iter()
        .find(|n| n.name == "WriteAsync" && n.qualified_name.contains("IWriter::"))
        .unwrap();
    let edges = cg.get_incoming_edges(&iface_method.id).await.unwrap();
    let from: BTreeSet<String> = edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Calls)
        .filter_map(|e| nodes.iter().find(|n| n.id == e.source))
        .map(|n| n.name.clone())
        .collect();
    assert!(
        from.contains("ExportAsync"),
        "Exporter.ExportAsync calls IWriter<T>.WriteAsync, got {from:?} (all WriteAsync callers in Writers.cs: {iface:?})"
    );

    let own = caller_names(&callers(&cg, "src/Export/Exporter.cs", "WriteAsync").await);
    assert!(
        own.is_empty(),
        "nothing calls Exporter.WriteAsync, got {own:?}"
    );

    let factory = caller_names(&callers(&cg, "src/Writing/Writers.cs", "CreateWriter").await);
    assert!(
        factory.contains("ExportAsync"),
        "a generic method call reaches CreateWriter, got {factory:?}"
    );
}

/// Fields, properties, explicitly typed locals, `new` and casts all type the
/// receiver; `this.` stays on the own class.
#[tokio::test]
async fn fields_properties_and_locals_type_the_receiver() {
    let (_dir, cg) = index(&all_files()).await;

    let refresher = callers(&cg, "src/Refresher.cs", "RefreshAsync").await;
    let run_lines: BTreeSet<u32> = refresher
        .iter()
        .filter(|(n, _)| n == "Run")
        .map(|(_, l)| *l)
        .collect();
    assert_eq!(
        run_lines.len(),
        3,
        "field, explicit local and cast calls reach Refresher.RefreshAsync, got {refresher:?}"
    );

    let coord = callers(&cg, "src/Coordinator.cs", "RefreshAsync").await;
    let run_lines: BTreeSet<u32> = coord
        .iter()
        .filter(|(n, _)| n == "Run")
        .map(|(_, l)| *l)
        .collect();
    assert_eq!(
        run_lines.len(),
        2,
        "property and `var x = new` calls reach Coordinator.RefreshAsync, got {coord:?}"
    );

    let own = callers(&cg, "src/Consumers/Consumer.cs", "RefreshAsync").await;
    assert_eq!(
        own.len(),
        1,
        "only `this.RefreshAsync()` reaches Consumer.RefreshAsync, got {own:?}"
    );
}

/// A receiver whose type has no such member (an extension method) still
/// resolves by name, as before.
#[tokio::test]
async fn extension_method_on_external_type_still_resolves() {
    let (_dir, cg) = index(&all_files()).await;
    let add = caller_names(&callers(&cg, "src/Hosting/Extensions.cs", "AddApp").await);
    assert!(
        add.contains("Configure"),
        "services.AddApp() reaches the extension method, got {add:?}"
    );
}

/// An incremental sync that adds the competing class agrees with a full index.
#[tokio::test]
async fn typed_receiver_edges_match_between_incremental_and_full_sync() {
    let files = all_files();
    let without: Vec<_> = files
        .iter()
        .copied()
        .filter(|(p, _)| *p != "src/Refresher.cs")
        .collect();
    let (inc_dir, inc) = index(&without).await;
    write(inc_dir.path(), "src/Refresher.cs", REFRESHER);
    inc.sync().await.unwrap();

    let (_full_dir, full) = index(&files).await;
    for (file, name) in [
        ("src/Coordinator.cs", "RefreshAsync"),
        ("src/Refresher.cs", "RefreshAsync"),
        ("src/Consumers/Consumer.cs", "RefreshAsync"),
    ] {
        let a = callers(&inc, file, name).await;
        let b = callers(&full, file, name).await;
        assert_eq!(a, b, "{file}::{name}: incremental and full sync disagree");
    }
}

/// A member inherited through the base list, an awaited factory
/// (`Task<T>` unwrapped) and a `where` clause after the base list.
#[tokio::test]
async fn inherited_members_and_awaited_factories_resolve() {
    let base = r#"namespace App.Store;

public abstract class StoreBase<T> where T : class
{
    public void Flush() { }
}

public sealed class Store<T> : StoreBase<T>, IDisposable where T : class
{
    public void Dispose() { }
}

public sealed class Opener
{
    public async Task<Store<string>> OpenAsync() => new Store<string>();
}

public sealed class Cache
{
    public void Flush() { }
}
"#;
    let user = r#"namespace App.Use;

public sealed class Client(Opener opener)
{
    public async Task Run()
    {
        var store = await opener.OpenAsync();
        store.Flush();
    }

    public void Flush() { }
}
"#;
    let (_dir, cg) = index(&[("src/Store/Store.cs", base), ("src/Use/Client.cs", user)]).await;

    let nodes = cg.get_all_nodes().await.unwrap();
    let flush = nodes
        .iter()
        .find(|n| n.name == "Flush" && n.qualified_name.contains("StoreBase::"))
        .unwrap();
    let from: BTreeSet<String> = cg
        .get_incoming_edges(&flush.id)
        .await
        .unwrap()
        .into_iter()
        .filter(|e| e.kind == EdgeKind::Calls)
        .filter_map(|e| nodes.iter().find(|n| n.id == e.source))
        .map(|n| n.name.clone())
        .collect();
    assert!(
        from.contains("Run"),
        "store.Flush() reaches StoreBase<T>.Flush, got {from:?}"
    );
    let own = caller_names(&callers(&cg, "src/Use/Client.cs", "Flush").await);
    assert!(own.is_empty(), "nothing calls Client.Flush, got {own:?}");
}

/// The extractor records the receiver's static type as a type expression.
#[test]
fn typed_call_refs_are_recorded() {
    let result = CSharpExtractor.extract("src/Export/Exporter.cs", EXPORTER);
    let names: Vec<&str> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .map(|r| r.reference_name.as_str())
        .collect();
    assert!(
        names.contains(&"IWriterFactory::CreateWriter"),
        "got {names:?}"
    );
    assert!(
        names.contains(&"IWriterFactory::CreateWriter()::WriteAsync"),
        "got {names:?}"
    );
    assert!(names.contains(&"writer.WriteAsync"), "got {names:?}");
    assert!(
        names.contains(&"writerFactory.CreateWriter"),
        "type arguments are not part of the callee name, got {names:?}"
    );
}

/// #670: a call through a field or property inherited from a base class
/// resolves through the member's declared type when that type lives in
/// another file than the call.
#[tokio::test]
async fn inherited_member_with_type_in_another_file_resolves() {
    let coordinator = r#"namespace Demo;

public sealed class RefreshCoordinator
{
    public Task RefreshAsync(string provider) => Task.CompletedTask;
}
"#;
    let tests = r#"namespace Demo;

public abstract class RefreshFixture
{
    protected RefreshCoordinator Coordinator { get; private set; } = null!;
    protected RefreshCoordinator _coordinator = null!;
    protected RefreshCoordinator FieldCoordinator = null!;
}

public class PollTests : RefreshFixture
{
    public async Task Polls()
    {
        await Coordinator.RefreshAsync("c");
    }

    public async Task PollsThroughField()
    {
        await _coordinator.RefreshAsync("f");
    }

    public async Task PollsThroughUpperField()
    {
        await FieldCoordinator.RefreshAsync("F");
    }
}
"#;
    // A class named like the inherited property: the member wins, as in C#.
    // A static call through a class name still resolves to the class.
    let shadow = r#"namespace Demo;

public static class Coordinator
{
    public static Task RefreshAsync(string provider) => Task.CompletedTask;
}

public class StaticUser
{
    public Task Use() => Coordinator.RefreshAsync("s");
}
"#;
    let files = [
        ("src/RefreshCoordinator.cs", coordinator),
        ("src/Coordinator.cs", shadow),
        ("tests/PollTests.cs", tests),
    ];

    let refs = CSharpExtractor.extract("tests/PollTests.cs", tests);
    let names: Vec<&str> = refs
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .map(|r| r.reference_name.as_str())
        .collect();
    assert!(
        names.contains(&"@Coordinator::RefreshAsync"),
        "an untyped identifier receiver is a simple-name root, got {names:?}"
    );

    let (_dir, cg) = index(&files).await;
    let from = caller_names(&callers(&cg, "src/RefreshCoordinator.cs", "RefreshAsync").await);
    for caller in ["Polls", "PollsThroughField", "PollsThroughUpperField"] {
        assert!(
            from.contains(caller),
            "{caller} calls RefreshCoordinator.RefreshAsync through an inherited member, got {from:?}"
        );
    }
    assert!(!from.contains("Use"), "got {from:?}");
    let static_from = caller_names(&callers(&cg, "src/Coordinator.cs", "RefreshAsync").await);
    assert_eq!(
        static_from,
        BTreeSet::from(["Use".to_string()]),
        "only the static call reaches the Coordinator class"
    );

    // An incremental sync that adds the base class's file agrees with a full
    // index: the inherited member's file is what the edge depends on.
    let (base, derived) = tests.split_at(tests.find("public class PollTests").unwrap());
    let (inc_dir, inc) = index(&[
        ("src/RefreshCoordinator.cs", coordinator),
        ("src/Coordinator.cs", shadow),
        (
            "tests/PollTests.cs",
            &format!("namespace Demo;\n\n{derived}"),
        ),
    ])
    .await;
    write(inc_dir.path(), "tests/RefreshFixture.cs", base);
    inc.sync().await.unwrap();
    let inc_from = caller_names(&callers(&inc, "src/RefreshCoordinator.cs", "RefreshAsync").await);
    assert_eq!(inc_from, from, "incremental and full sync disagree");
}

/// #670 follow-up: a variable the extractor cannot type (a lambda, `out`,
/// pattern, catch, `foreach` or query variable, or a `var` local with an
/// opaque initializer) still hides an inherited member of the same name, so
/// a call on it never binds through that member's type.
#[tokio::test]
async fn untyped_locals_hide_inherited_members() {
    let coordinator = r#"namespace Demo;

public sealed class RefreshCoordinator
{
    public Task RefreshAsync(string provider) => Task.CompletedTask;
}
"#;
    let widget = r#"namespace Demo;

public sealed class Widget
{
    public Task RefreshAsync(string provider) => Task.CompletedTask;
}
"#;
    let fixture = r#"namespace Demo;

public abstract class RefreshFixture
{
    protected RefreshCoordinator item = null!;
    protected RefreshCoordinator @event = null!;
}
"#;
    let tests = r#"namespace Demo;

public class PollTests : RefreshFixture
{
    public void ViaLambda(List<Widget> widgets)
    {
        widgets.ForEach(item => item.RefreshAsync("l"));
    }

    public void ViaOut(Dictionary<string, Widget> map)
    {
        if (map.TryGetValue("k", out var item)) { item.RefreshAsync("o"); }
    }

    public void ViaPattern(object o)
    {
        if (o is var item) { item.RefreshAsync("p"); }
    }

    public void ViaForeach(IEnumerable<Widget> widgets)
    {
        foreach (var item in widgets) { item.RefreshAsync("e"); }
    }

    public void ViaQuery(IEnumerable<Widget> widgets)
    {
        var all = from item in widgets select item.RefreshAsync("q");
    }

    public void ViaOpaqueVar(IEnumerable<Widget> widgets)
    {
        var item = widgets.First();
        item.RefreshAsync("v");
    }

    public void ViaInherited()
    {
        item.RefreshAsync("i");
    }

    public void ViaVerbatim()
    {
        @event.RefreshAsync("verbatim");
    }
}
"#;
    let refs = CSharpExtractor.extract("tests/PollTests.cs", tests);
    let typed: Vec<&str> = refs
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .map(|r| r.reference_name.as_str())
        .filter(|n| n.starts_with('@'))
        .collect();
    assert!(
        typed.contains(&"@item::RefreshAsync"),
        "the inherited-member call is still a simple-name root, got {typed:?}"
    );
    assert!(
        typed.contains(&"@event::RefreshAsync"),
        "the verbatim prefix is not part of the name, got {typed:?}"
    );

    let (_dir, cg) = index(&[
        ("src/RefreshCoordinator.cs", coordinator),
        ("src/Widget.cs", widget),
        ("tests/RefreshFixture.cs", fixture),
        ("tests/PollTests.cs", tests),
    ])
    .await;
    let from = caller_names(&callers(&cg, "src/RefreshCoordinator.cs", "RefreshAsync").await);
    for local in [
        "ViaLambda",
        "ViaOut",
        "ViaPattern",
        "ViaForeach",
        "ViaQuery",
        "ViaOpaqueVar",
    ] {
        assert!(
            !from.contains(local),
            "{local} calls through a local, not the inherited field, got {from:?}"
        );
    }
    assert!(from.contains("ViaInherited"), "got {from:?}");
    assert!(from.contains("ViaVerbatim"), "got {from:?}");
}
