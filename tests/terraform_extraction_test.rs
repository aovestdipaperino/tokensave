#![cfg(feature = "lang-terraform")]

use tokensave::extraction::{LanguageExtractor, TerraformExtractor};
use tokensave::types::{EdgeKind, ExtractionResult, NodeKind};

fn extract(path: &str, source: &str) -> ExtractionResult {
    TerraformExtractor.extract(path, source)
}

fn names(result: &ExtractionResult, kind: NodeKind) -> Vec<String> {
    result
        .nodes
        .iter()
        .filter(|node| node.kind == kind)
        .map(|node| node.name.clone())
        .collect()
}

#[test]
fn extracts_terraform_blocks_with_canonical_names() {
    let result = extract(
        "main.tf",
        r#"
terraform {
  required_version = ">= 1.6"
}

provider "aws" {
  region = var.region
}

variable "region" {
  type = string
}

locals {
  tags = { app = "demo" }
}

data "aws_ami" "base" {
  most_recent = true
}

resource "aws_instance" "web" {
  ami = data.aws_ami.base.id
}

module "network" {
  source = "./network"
}

output "instance_id" {
  value = aws_instance.web.id
}
"#,
    );

    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(
        names(&result, NodeKind::Module),
        vec![
            "terraform",
            "provider.aws",
            "var.region",
            "locals",
            "data.aws_ami.base",
            "resource.aws_instance.web",
            "module.network",
            "output.instance_id",
        ]
    );
    assert!(names(&result, NodeKind::Const).contains(&"local.tags".to_string()));
}

#[test]
fn direct_attributes_are_contained_by_their_blocks() {
    let result = extract(
        "main.tf",
        r#"
resource "aws_instance" "web" {
  ami           = "ami-123"
  instance_type = "t3.micro"
}
"#,
    );
    let block = result
        .nodes
        .iter()
        .find(|node| node.name == "resource.aws_instance.web")
        .unwrap();

    for attribute in ["ami", "instance_type"] {
        let node = result
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Const && node.name == attribute)
            .unwrap();
        assert!(result.edges.iter().any(|edge| {
            edge.kind == EdgeKind::Contains && edge.source == block.id && edge.target == node.id
        }));
    }
}

#[test]
fn tfvars_assignments_use_bare_names() {
    let result = extract(
        "production.tfvars",
        r#"
region = "eu-west-1"
count  = 3
"#,
    );

    assert_eq!(names(&result, NodeKind::Const), vec!["region", "count"]);
    assert!(!result.nodes.iter().any(|node| node.name == "var.region"));
}

#[test]
fn unknown_top_level_blocks_are_not_claimed_as_terraform_declarations() {
    let result = extract(
        "main.tf",
        r#"
dynamic "setting" {
  content {
    value = true
  }
}
"#,
    );

    assert!(names(&result, NodeKind::Module).is_empty());
}

#[test]
fn normalizes_terraform_references_without_text_false_positives() {
    let result = extract(
        "main.tf",
        r#"
resource "aws_instance" "web" {
  ami        = data.aws_ami.base.id
  subnet_id  = module.network.subnet_id
  tags       = local.tags
  peer       = aws_instance.peer.id
  region     = var.region
  item_count = length(var.items)
  path       = path.module
  workspace  = terraform.workspace
  ordinal    = count.index
  key        = each.key
  own_id     = self.id
  literal    = "module.fake.id"
  script = <<-EOT
    module.not_a_reference.id
    ${var.from_template}
  EOT

  # data.fake.comment.id
}
"#,
    );

    let mut references: Vec<_> = result
        .unresolved_refs
        .iter()
        .map(|reference| reference.reference_name.as_str())
        .collect();
    references.sort_unstable();
    assert_eq!(
        references,
        vec![
            "data.aws_ami.base",
            "local.tags",
            "module.network",
            "resource.aws_instance.peer",
            "var.from_template",
            "var.items",
            "var.region",
        ]
    );
    assert!(result
        .unresolved_refs
        .iter()
        .all(|reference| reference.reference_kind == EdgeKind::Uses));
}

#[test]
fn keeps_valid_neighboring_blocks_when_input_is_malformed() {
    let result = extract(
        "main.tf",
        r#"
resource "broken" {
  value =
}

output "ok" {
  value = var.region
}
"#,
    );

    assert!(result.nodes.iter().any(|node| node.name == "output.ok"));
    assert!(result
        .unresolved_refs
        .iter()
        .any(|reference| reference.reference_name == "var.region"));
}
