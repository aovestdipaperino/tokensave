// XAML / AXAML extraction (#167): Avalonia and WPF views must be indexed.
#![cfg(feature = "lang-xaml")]

use tokensave::extraction::LanguageExtractor;
use tokensave::extraction::XamlExtractor;
use tokensave::types::*;

const AVALONIA_VIEW: &str = r#"<UserControl xmlns="https://github.com/avaloniaui"
             xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
             xmlns:d="http://schemas.microsoft.com/expression/blend/2008"
             xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"
             xmlns:controls="using:MyApp.Controls"
             mc:Ignorable="d"
             x:Class="MyApp.Views.MainView">
  <!-- a comment with <Button Click="NotReal"/> inside -->
  <StackPanel>
    <Menu x:Name="MainMenu">
      <MenuItem Header="File" Click="OnFileClicked" />
    </Menu>
    <controls:TitleBar />
    <Button x:Name="SaveButton" Content="Save" Click="OnSaveClicked" />
    <TextBox Name="SearchBox" TextChanged="OnSearchChanged" Text="{Binding Query}" />
    <Grid Grid.Row="1" />
  </StackPanel>
</UserControl>
"#;

fn extract(source: &str) -> ExtractionResult {
    XamlExtractor.extract("Views/MainView.axaml", source)
}

#[test]
fn test_xaml_extensions_registered() {
    let registry = tokensave::extraction::LanguageRegistry::new();
    let exts = registry.supported_extensions();
    assert!(exts.contains(&"xaml"), "xaml not registered: {exts:?}");
    assert!(exts.contains(&"axaml"), "axaml not registered: {exts:?}");
}

#[test]
fn test_xaml_class_node_from_x_class() {
    let result = extract(AVALONIA_VIEW);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    let class = result
        .nodes
        .iter()
        .find(|n| n.kind == NodeKind::Class)
        .expect("expected a Class node from x:Class");
    assert_eq!(class.name, "MainView");
    assert_eq!(class.qualified_name, "MyApp.Views.MainView");
}

#[test]
fn test_xaml_named_elements_become_fields() {
    let result = extract(AVALONIA_VIEW);
    let fields: Vec<_> = result
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Field)
        .map(|n| n.name.as_str())
        .collect();
    assert!(fields.contains(&"MainMenu"), "fields: {fields:?}");
    assert!(fields.contains(&"SaveButton"), "fields: {fields:?}");
    assert!(fields.contains(&"SearchBox"), "fields: {fields:?}");
}

#[test]
fn test_xaml_event_handlers_emit_calls_refs() {
    let result = extract(AVALONIA_VIEW);
    let calls: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Calls)
        .map(|r| r.reference_name.as_str())
        .collect();
    assert!(calls.contains(&"OnFileClicked"), "calls: {calls:?}");
    assert!(calls.contains(&"OnSaveClicked"), "calls: {calls:?}");
    assert!(calls.contains(&"OnSearchChanged"), "calls: {calls:?}");
    // Bindings and commented-out handlers must not leak.
    assert!(!calls.contains(&"NotReal"), "calls: {calls:?}");
}

#[test]
fn test_xaml_custom_controls_emit_uses_refs() {
    let result = extract(AVALONIA_VIEW);
    let uses: Vec<_> = result
        .unresolved_refs
        .iter()
        .filter(|r| r.reference_kind == EdgeKind::Uses)
        .map(|r| r.reference_name.as_str())
        .collect();
    assert!(uses.contains(&"TitleBar"), "uses: {uses:?}");
    // Unprefixed framework controls must not flood the graph.
    assert!(!uses.contains(&"Button"), "uses: {uses:?}");
    assert!(!uses.contains(&"StackPanel"), "uses: {uses:?}");
}

#[test]
fn test_xaml_without_x_class_still_indexes() {
    // Resource dictionaries have no x:Class; named elements parent to the file.
    let result = extract(
        "<ResourceDictionary xmlns=\"https://github.com/avaloniaui\">\n  \
         <SolidColorBrush x:Name=\"AccentBrush\" Color=\"Red\" />\n</ResourceDictionary>",
    );
    assert!(result.errors.is_empty());
    assert!(result.nodes.iter().any(|n| n.kind == NodeKind::File));
    assert!(!result.nodes.iter().any(|n| n.kind == NodeKind::Class));
    assert!(result
        .nodes
        .iter()
        .any(|n| n.kind == NodeKind::Field && n.name == "AccentBrush"));
}

#[test]
fn test_xaml_binding_values_are_not_handlers() {
    let result = extract(
        "<UserControl x:Class=\"App.V\" xmlns:x=\"x\">\n  \
         <Button Click=\"{Binding SaveCommand}\" />\n</UserControl>",
    );
    assert!(
        !result
            .unresolved_refs
            .iter()
            .any(|r| r.reference_kind == EdgeKind::Calls),
        "binding expressions must not emit Calls refs: {:?}",
        result.unresolved_refs
    );
}
