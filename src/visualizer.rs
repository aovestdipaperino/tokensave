//! Interactive graph visualizer served on localhost.
//!
//! `tokensave visualize` launches a lightweight HTTP server that serves a
//! Cytoscape.js frontend and exposes REST API endpoints backed by the
//! TokenSave knowledge graph.

use std::collections::HashMap;
use std::io::Write;
use std::net::TcpListener;

use crate::errors::{Result, TokenSaveError};
use crate::tokensave::TokenSave;

/// Percent-decode a URL-encoded string (e.g. `%3A` → `:`).
fn url_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let hi = chars.next().unwrap_or(b'0');
            let lo = chars.next().unwrap_or(b'0');
            let hex = [hi, lo];
            if let Ok(decoded) = u8::from_str_radix(
                std::str::from_utf8(&hex).unwrap_or("00"),
                16,
            ) {
                result.push(decoded as char);
            }
        } else if b == b'+' {
            result.push(' ');
        } else {
            result.push(b as char);
        }
    }
    result
}

/// Start the visualizer server and open the browser.
pub async fn run(cg: &TokenSave, port: u16) -> Result<()> {
    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).map_err(|e| {
        TokenSaveError::Config {
            message: format!("failed to bind port {port}: {e}"),
        }
    })?;
    let addr = listener.local_addr().map_err(|e| TokenSaveError::Config {
        message: format!("failed to get local addr: {e}"),
    })?;
    let url = format!("http://{addr}");

    eprintln!("Visualizer running at \x1b[1m{url}\x1b[0m");
    eprintln!("Press Ctrl+C to stop.\n");

    // Best-effort open browser.
    #[cfg(target_os = "macos")]
    { let _ = std::process::Command::new("open").arg(&url).spawn(); }
    #[cfg(target_os = "linux")]
    { let _ = std::process::Command::new("xdg-open").arg(&url).spawn(); }
    #[cfg(target_os = "windows")]
    { let _ = std::process::Command::new("cmd").args(["/C", "start", &url]).spawn(); }

    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        if let Err(e) = handle_connection(cg, &mut stream).await {
            eprintln!("[visualizer] error: {e}");
        }
    }

    Ok(())
}

async fn handle_connection(
    cg: &TokenSave,
    stream: &mut std::net::TcpStream,
) -> Result<()> {
    use std::io::{BufRead, BufReader};

    let mut reader = BufReader::new(stream.try_clone().map_err(|e| TokenSaveError::Config {
        message: format!("clone: {e}"),
    })?);

    let mut request_line = String::new();
    reader.read_line(&mut request_line).map_err(|e| TokenSaveError::Config {
        message: format!("read: {e}"),
    })?;

    // Parse "GET /path?query HTTP/1.1"
    let parts: Vec<&str> = request_line.trim().split_whitespace().collect();
    if parts.len() < 2 {
        return Ok(());
    }
    let full_path = parts[1];
    let (path, query_string) = full_path.split_once('?').unwrap_or((full_path, ""));
    let query = parse_query(query_string);

    // Drain headers
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).map_err(|e| TokenSaveError::Config {
            message: format!("read header: {e}"),
        })?;
        if line.trim().is_empty() {
            break;
        }
    }

    match path {
        "/" | "/index.html" => serve_html(stream),
        "/api/status" => {
            let stats = cg.get_stats().await?;
            let project = cg.project_root().display().to_string();
            let body = serde_json::json!({
                "stats": stats,
                "projectRoot": project,
            });
            serve_json(stream, &body)
        }
        "/api/search" => {
            let q = query.get("q").map_or("", |s| s.as_str());
            let limit: usize = query.get("limit").and_then(|s| s.parse().ok()).unwrap_or(30);
            if q.is_empty() {
                return serve_json(stream, &serde_json::json!({ "results": [] }));
            }
            let results = cg.search(q, limit).await?;
            serve_json(stream, &serde_json::json!({ "results": results }))
        }
        "/api/explore" => {
            let q = query.get("q").map_or("", |s| s.as_str());
            if q.is_empty() {
                return serve_json(stream, &serde_json::json!({
                    "nodes": [], "edges": [], "roots": []
                }));
            }
            // Find best entry point via search
            let results = cg.search(q, 5).await?;
            if let Some(first) = results.first() {
                let call_graph = cg.get_call_graph(&first.node.id, 3).await?;
                serve_json(stream, &serde_json::json!({
                    "nodes": call_graph.nodes,
                    "edges": call_graph.edges,
                    "roots": [first.node.id],
                    "entryPoint": first.node.id,
                }))
            } else {
                serve_json(stream, &serde_json::json!({
                    "nodes": [], "edges": [], "roots": []
                }))
            }
        }
        p if p.starts_with("/api/node/") => {
            let rest = &p["/api/node/".len()..];
            let (node_id, sub) = rest.split_once('/').unwrap_or((rest, ""));
            let node_id = url_decode(node_id);

            match sub {
                "" => {
                    let node = cg.get_node(&node_id).await?;
                    serve_json(stream, &serde_json::json!({ "node": node }))
                }
                "callers" => {
                    let depth: usize = query.get("depth").and_then(|s| s.parse().ok()).unwrap_or(1);
                    let pairs = cg.get_callers(&node_id, depth).await?;
                    let nodes: Vec<_> = pairs.iter().map(|(n, _)| n).collect();
                    let edges: Vec<_> = pairs.iter().map(|(_, e)| e).collect();
                    serve_json(stream, &serde_json::json!({ "nodes": nodes, "edges": edges, "roots": [node_id] }))
                }
                "callees" => {
                    let depth: usize = query.get("depth").and_then(|s| s.parse().ok()).unwrap_or(1);
                    let pairs = cg.get_callees(&node_id, depth).await?;
                    let nodes: Vec<_> = pairs.iter().map(|(n, _)| n).collect();
                    let edges: Vec<_> = pairs.iter().map(|(_, e)| e).collect();
                    serve_json(stream, &serde_json::json!({ "nodes": nodes, "edges": edges, "roots": [node_id] }))
                }
                "impact" => {
                    let depth: usize = query.get("depth").and_then(|s| s.parse().ok()).unwrap_or(2);
                    let sg = cg.get_impact_radius(&node_id, depth).await?;
                    serve_json(stream, &serde_json::json!(sg))
                }
                "callgraph" => {
                    let depth: usize = query.get("depth").and_then(|s| s.parse().ok()).unwrap_or(2);
                    let sg = cg.get_call_graph(&node_id, depth).await?;
                    serve_json(stream, &serde_json::json!(sg))
                }
                _ => serve_404(stream),
            }
        }
        _ => serve_404(stream),
    }
}

fn parse_query(qs: &str) -> HashMap<String, String> {
    qs.split('&')
        .filter(|s| !s.is_empty())
        .filter_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            Some((
                url_decode(k),
                url_decode(v),
            ))
        })
        .collect()
}

fn serve_json(stream: &mut std::net::TcpStream, value: &serde_json::Value) -> Result<()> {
    let body = serde_json::to_string(value).unwrap_or_default();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(response.as_bytes()).map_err(|e| TokenSaveError::Config {
        message: format!("write: {e}"),
    })?;
    Ok(())
}

fn serve_html(stream: &mut std::net::TcpStream) -> Result<()> {
    let body = include_str!("visualizer_index.html");
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(response.as_bytes()).map_err(|e| TokenSaveError::Config {
        message: format!("write: {e}"),
    })?;
    Ok(())
}

fn serve_404(stream: &mut std::net::TcpStream) -> Result<()> {
    let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\nConnection: close\r\n\r\nNot found";
    stream.write_all(response.as_bytes()).map_err(|e| TokenSaveError::Config {
        message: format!("write: {e}"),
    })?;
    Ok(())
}
