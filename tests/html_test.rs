use lemmaspec::{project_artifact, render_projection_html};

const EXAMPLE: &str = include_str!("../examples/release_readiness.lemmaspec");
const PERSISTENCE_ADAPTER: &str =
    include_str!("../examples/persistence_adapter_readiness.lemmaspec");
const PERSISTENCE_ADAPTER_HTML: &str =
    include_str!("../examples/persistence_adapter_readiness.html");
const MUTATION_ANALYSIS: &str = include_str!("../examples/mutation_analysis.lemmaspec");

#[test]
fn renders_a_dependency_free_document_with_the_canonical_graph_embedded() {
    let projection = project_artifact(EXAMPLE).expect("project example");
    let html = render_projection_html(EXAMPLE, &projection);

    assert!(html.starts_with("<!doctype html>"), "{html}");
    assert!(html.contains("Release readiness"), "{html}");
    assert!(html.contains("All 1 expectation is satisfied"), "{html}");
    assert!(html.contains("id=\"lemmaspec-graph\""), "{html}");
    assert!(html.contains("id=\"artifact-source\""), "{html}");
    assert!(
        html.contains("including proof witnesses and evidence"),
        "{html}"
    );
    assert!(html.contains("\"witness\":\"blocked_by_incomplete_dependency\""));
    assert!(!html.contains("<script src="), "{html}");
    assert!(!html.contains("https://"), "{html}");

    let json = between(
        &html,
        "<script type=\"application/json\" id=\"lemmaspec-graph\">",
        "</script>",
    );
    let embedded: serde_json::Value = serde_json::from_str(json).expect("embedded graph JSON");
    assert_eq!(
        embedded,
        serde_json::to_value(&projection).expect("projection JSON")
    );
}

#[test]
fn escapes_source_and_embedded_json_across_html_script_boundaries() {
    let source = r#"
spec escaping {
  relation input { args: [symbol] }
  fact hostile { relation: input args: ["</script><script>alert(1)</script>"] }
  expect input_exists { query: "input(Value)" count: 1 }
}
"#;
    let projection = project_artifact(source).expect("project hostile source");
    let html = render_projection_html(source, &projection);

    assert!(!html.contains("</script><script>alert(1)</script>"));
    assert!(html.contains("&lt;/script&gt;&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(html.contains("\\u003c/script\\u003e\\u003cscript\\u003ealert(1)"));
}

#[test]
fn repeated_html_projection_is_byte_stable() {
    let projection = project_artifact(EXAMPLE).expect("project example");

    assert_eq!(
        render_projection_html(EXAMPLE, &projection),
        render_projection_html(EXAMPLE, &projection)
    );
}

#[test]
fn renders_mutation_policies_as_constraints() {
    let projection = project_artifact(MUTATION_ANALYSIS).expect("project mutation analysis");
    let html = render_projection_html(MUTATION_ANALYSIS, &projection);

    assert!(html.contains("data-kind=\"mutation\""), "{html}");
    assert!(html.contains("semantic_rules_are_load_bearing"), "{html}");
    assert!(html.contains("<code>drop_rule</code>"), "{html}");
    assert!(
        html.contains("<li><code>mutation_exists: mutation(Mutation)</code></li>"),
        "{html}"
    );
}

#[test]
fn committed_persistence_adapter_view_matches_its_incomplete_projection() {
    let projection =
        project_artifact(PERSISTENCE_ADAPTER).expect("project persistence adapter example");

    assert_eq!(projection.status, "incomplete");

    let html = render_projection_html(PERSISTENCE_ADAPTER, &projection);
    assert_eq!(html, PERSISTENCE_ADAPTER_HTML);

    let json = between(
        &html,
        "<script type=\"application/json\" id=\"lemmaspec-graph\">",
        "</script>",
    );
    let embedded: serde_json::Value = serde_json::from_str(json).expect("embedded graph JSON");
    assert_eq!(
        embedded,
        serde_json::to_value(&projection).expect("projection JSON")
    );
}

fn between<'a>(text: &'a str, start: &str, end: &str) -> &'a str {
    let (_, tail) = text.split_once(start).expect("start marker");
    let (value, _) = tail.split_once(end).expect("end marker");
    value
}
