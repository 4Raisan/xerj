//! `xerj_status`: is this node up, and what is on it? (#2515)
//!
//! The field report that opened #2515 probed a node by calling `xerj_map`,
//! which answered 404 "no autoindex catalog". Read from an agent's side a 404
//! is hard to tell apart from a connection that never landed, so the reporter
//! fell back to `netstat` and a hand-written `GET /` to answer "is it running".
//!
//! This tool answers that question directly, and separates the states that
//! were previously one error: not reachable, reachable but refusing the
//! credentials, reachable and empty, reachable with data.
//!
//! It is deliberately cheap — three `GET`s, no search, no catalog read — so it
//! can be the first call of a session without spending the budget a real query
//! needs. The answer is a few lines of text.

use serde_json::Value;

use crate::map::CATALOG_INDEX;
use crate::{tool_text, transport_failure_text, Ctx};

/// Index rows printed before the rest are summarized as a count, so a node
/// with hundreds of indices still answers in a readable number of lines.
const MAX_LISTED_INDICES: usize = 20;

/// One `GET`, keeping the status even when it is an error: for a liveness
/// probe a 404 or a 401 is an answer, not a failure.
async fn get(ctx: &Ctx, path: &str) -> Result<(u16, String), (String, reqwest::Error)> {
    let url = format!("{}{}", ctx.base_url, path);
    let mut req = ctx.client.get(&url);
    if let Some(auth) = &ctx.auth {
        req = req.header("Authorization", auth);
    }
    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            Ok((status, resp.text().await.unwrap_or_default()))
        }
        Err(e) => Err((url, e)),
    }
}

fn parse(text: &str) -> Option<Value> {
    serde_json::from_str(text).ok()
}

pub(crate) async fn run(ctx: &Ctx, _args: &Value) -> Value {
    let (status, banner_text) = match get(ctx, "/").await {
        Ok(ok) => ok,
        Err((url, e)) => {
            return tool_text(
                format!(
                    "xerj_status: no node is answering at {}.\n{}",
                    ctx.base_url,
                    transport_failure_text(&ctx.base_url, &url, &e)
                ),
                true,
            );
        }
    };

    // A 401/403 answers the liveness question ("up") but nothing else will
    // work, so say that plainly instead of reporting an empty node.
    if status == 401 || status == 403 {
        return tool_text(
            format!(
                "xerj_status: the node at {} is UP but refused these credentials \
                 (root endpoint answered HTTP {status}).\n\
                 This is an auth problem, not a connection problem: queries will fail \
                 the same way. Point XERJ_AUTH (or `xerj mcp --auth`) at a key this node \
                 accepts, or ask its owner for the admin key.",
                ctx.base_url
            ),
            true,
        );
    }

    let banner = parse(&banner_text);
    let es_version = banner
        .as_ref()
        .and_then(|b| b.get("version"))
        .and_then(|v| v.get("number"))
        .and_then(Value::as_str);
    let cluster_name = banner
        .as_ref()
        .and_then(|b| b.get("cluster_name"))
        .and_then(Value::as_str);

    let mut lines = vec![format!(
        "xerj_status: the node at {} is UP (root endpoint answered HTTP {status}).",
        ctx.base_url
    )];
    match es_version {
        Some(v) => lines.push(format!(
            "  version: {v} (the Elasticsearch-compatible number this node reports, \
             not XERJ's own version)"
        )),
        None => lines.push(
            "  version: not reported — the root endpoint did not return a JSON banner".into(),
        ),
    }
    if let Some(name) = cluster_name {
        lines.push(format!("  cluster name: {name}"));
    }

    match get(ctx, "/_cluster/health").await {
        Ok((code, text)) => match parse(&text) {
            Some(h) if (200..300).contains(&code) => {
                let state = h.get("status").and_then(Value::as_str).unwrap_or("unknown");
                let nodes = h.get("number_of_nodes").and_then(Value::as_u64);
                let shards = h.get("active_shards").and_then(Value::as_u64);
                let mut facts = Vec::new();
                if let Some(n) = nodes {
                    facts.push(format!("{n} node{}", if n == 1 { "" } else { "s" }));
                }
                if let Some(s) = shards {
                    facts.push(format!("{s} active shards"));
                }
                lines.push(if facts.is_empty() {
                    format!("  cluster health: {state}")
                } else {
                    format!("  cluster health: {state} ({})", facts.join(", "))
                });
            }
            _ => lines.push(format!(
                "  cluster health: unavailable (GET /_cluster/health answered HTTP {code})"
            )),
        },
        Err((url, e)) => lines.push(format!(
            "  cluster health: unavailable ({})",
            transport_failure_text(&ctx.base_url, &url, &e)
        )),
    }

    let indices = match get(ctx, "/_cat/indices?format=json&h=index,docs.count").await {
        Ok((code, text)) if (200..300).contains(&code) => parse(&text)
            .and_then(|v| v.as_array().cloned())
            .map(|rows| {
                rows.iter()
                    .filter_map(|r| {
                        let name = r.get("index").and_then(Value::as_str)?;
                        let docs = r.get("docs.count").and_then(|d| {
                            d.as_str()
                                .and_then(|s| s.parse::<u64>().ok())
                                .or_else(|| d.as_u64())
                        });
                        Some((name.to_string(), docs))
                    })
                    .collect::<Vec<_>>()
            }),
        _ => None,
    };

    match &indices {
        Some(rows) if rows.is_empty() => {
            lines.push("  indices: none".into());
        }
        Some(rows) => {
            lines.push(format!(
                "  indices: {}{}",
                rows.len(),
                if rows.len() > MAX_LISTED_INDICES {
                    format!(" (first {MAX_LISTED_INDICES} listed)")
                } else {
                    String::new()
                }
            ));
            let width = rows
                .iter()
                .take(MAX_LISTED_INDICES)
                .map(|(n, _)| n.len())
                .max()
                .unwrap_or(0);
            for (name, docs) in rows.iter().take(MAX_LISTED_INDICES) {
                match docs {
                    Some(d) => lines.push(format!("    {name:<width$}  {d} docs")),
                    None => lines.push(format!("    {name}")),
                }
            }
        }
        None => lines
            .push("  indices: unavailable (GET /_cat/indices did not return a JSON list)".into()),
    }

    // The point of the whole tool: an empty node must not read as a broken
    // one. `xerj_map` answers 404 "no autoindex catalog" here, and that is
    // the node saying it has nothing indexed — not a connectivity failure.
    match &indices {
        Some(rows) => {
            let catalog = rows.iter().find(|(name, _)| name == CATALOG_INDEX);
            match catalog {
                Some((_, docs)) => lines.push(format!(
                    "  autoindex catalog: PRESENT (`{CATALOG_INDEX}`{}, so xerj_map and \
                     xerj_search have data to read).",
                    match docs {
                        Some(d) => format!(" holds {d} docs"),
                        None => String::new(),
                    }
                )),
                None => {
                    lines.push(
                        "  autoindex catalog: ABSENT — this node is running and has nothing \
                         indexed yet."
                            .into(),
                    );
                    lines.push(
                        "    xerj_map and xerj_search answer \"no autoindex catalog\" on this \
                         node. That is an empty node, not a connection failure: index a folder \
                         with `xerj autoindex <folder>`, then call this tool again."
                            .into(),
                    );
                }
            }
        }
        None => lines.push(format!(
            "  autoindex catalog: unknown — the index list could not be read, so whether \
             `{CATALOG_INDEX}` exists is not answered here."
        )),
    }

    tool_text(lines.join("\n"), false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    /// Serve a fixed route table on a loopback port. Keys are paths without
    /// their query string, because the real calls carry `?format=json&...`.
    async fn serve(routes: HashMap<&'static str, (u16, String)>) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    return;
                };
                let mut buf = vec![0u8; 8192];
                let n = tokio::io::AsyncReadExt::read(&mut sock, &mut buf)
                    .await
                    .unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]).to_string();
                let target = request.split_whitespace().nth(1).unwrap_or("/");
                let path = target.split('?').next().unwrap_or("/").to_string();
                let (status, body) = routes
                    .get(path.as_str())
                    .cloned()
                    .unwrap_or((404, "{}".to_string()));
                let reply = format!(
                    "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\
                     Content-Type: application/json\r\n\r\n{body}",
                    body.len()
                );
                let _ = tokio::io::AsyncWriteExt::write_all(&mut sock, reply.as_bytes()).await;
            }
        });
        format!("http://{addr}")
    }

    fn ctx(base_url: String) -> Ctx {
        Ctx {
            client: reqwest::Client::new(),
            base_url,
            auth: None,
        }
    }

    fn banner() -> String {
        json!({
            "name": "node-1",
            "cluster_name": "xerj",
            "version": { "number": "8.13.0", "distribution": "xerj" },
            "tagline": "You Know, for Search"
        })
        .to_string()
    }

    fn health(state: &str) -> String {
        json!({
            "cluster_name": "xerj",
            "status": state,
            "number_of_nodes": 1,
            "active_shards": 15
        })
        .to_string()
    }

    fn cat(rows: Value) -> String {
        rows.to_string()
    }

    fn text(v: &Value) -> String {
        v["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    }

    /// The bug in #2515: `xerj_map` 404s on an unindexed node, and from the
    /// agent's side that reads like the node is not there. The status tool has
    /// to say UP and EMPTY in one answer, and name the 404 as the empty case.
    #[tokio::test]
    async fn an_unindexed_node_reports_up_and_empty_not_unreachable() {
        let mut routes = HashMap::new();
        routes.insert("/", (200, banner()));
        routes.insert("/_cluster/health", (200, health("green")));
        routes.insert("/_cat/indices", (200, cat(json!([]))));
        let base = serve(routes).await;

        let out = run(&ctx(base), &json!({})).await;
        assert_eq!(out["isError"], false, "{}", text(&out));
        let body = text(&out);
        assert!(body.contains("is UP"), "{body}");
        assert!(body.contains("indices: none"), "{body}");
        assert!(body.contains("autoindex catalog: ABSENT"), "{body}");
        assert!(
            body.contains("not a connection failure"),
            "the empty node must not read as a broken one: {body}"
        );
        assert!(body.contains("xerj autoindex <folder>"), "{body}");
        assert!(body.contains("8.13.0"), "{body}");
        assert!(body.contains("green"), "{body}");
    }

    #[tokio::test]
    async fn an_indexed_node_names_the_catalog_and_its_doc_count() {
        let mut routes = HashMap::new();
        routes.insert("/", (200, banner()));
        routes.insert("/_cluster/health", (200, health("yellow")));
        routes.insert(
            "/_cat/indices",
            (
                200,
                cat(json!([
                    { "index": "autoindex-catalog", "docs.count": "12" },
                    { "index": "ax", "docs.count": "812" }
                ])),
            ),
        );
        let base = serve(routes).await;

        let out = run(&ctx(base), &json!({})).await;
        assert_eq!(out["isError"], false, "{}", text(&out));
        let body = text(&out);
        assert!(body.contains("indices: 2"), "{body}");
        assert!(body.contains("autoindex-catalog"), "{body}");
        assert!(body.contains("12 docs"), "{body}");
        assert!(body.contains("autoindex catalog: PRESENT"), "{body}");
        assert!(!body.contains("ABSENT"), "{body}");
        assert!(body.contains("yellow"), "{body}");
    }

    /// Nothing listening: the one case that *is* a connectivity failure, and it
    /// has to say so rather than printing an empty successful answer.
    #[tokio::test]
    async fn nothing_listening_is_the_only_unreachable_answer() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let out = run(&ctx(format!("http://{addr}")), &json!({})).await;
        assert_eq!(out["isError"], true, "{}", text(&out));
        let body = text(&out);
        assert!(body.contains("no node is answering"), "{body}");
    }

    /// A 401 means the node is running. Reporting that as "unreachable" would
    /// send an agent looking at ports when the problem is the key.
    #[tokio::test]
    async fn a_refused_key_is_up_with_an_auth_problem_not_down() {
        let mut routes = HashMap::new();
        routes.insert("/", (401, "{\"error\":\"unauthorized\"}".to_string()));
        let base = serve(routes).await;

        let out = run(&ctx(base), &json!({})).await;
        let body = text(&out);
        assert!(body.contains("is UP but refused"), "{body}");
        assert!(
            body.contains("auth problem, not a connection problem"),
            "{body}"
        );
        assert!(body.contains("XERJ_AUTH"), "{body}");
    }

    /// A node that answers the banner but not `_cat/indices` is up. The catalog
    /// question is then unknown, which must not be reported as absent.
    #[tokio::test]
    async fn an_unreadable_index_list_is_unknown_not_absent() {
        let mut routes = HashMap::new();
        routes.insert("/", (200, banner()));
        routes.insert("/_cluster/health", (200, health("green")));
        routes.insert("/_cat/indices", (500, "boom".to_string()));
        let base = serve(routes).await;

        let out = run(&ctx(base), &json!({})).await;
        assert_eq!(out["isError"], false, "{}", text(&out));
        let body = text(&out);
        assert!(body.contains("autoindex catalog: unknown"), "{body}");
        assert!(!body.contains("ABSENT"), "{body}");
    }
}
