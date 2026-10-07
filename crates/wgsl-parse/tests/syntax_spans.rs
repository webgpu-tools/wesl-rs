//! Tests for rendering syntax to text with the ranges of its spanned nodes.

use pretty::DocAllocator;
use wgsl_parse::{Alloc, PrintedSpan, parse_str, print_with_spans, syntax::TranslationUnit};

const KITCHEN_SINK: &str = "
enable f16;

alias Float = f32;

struct Light {
    @size(16) position: vec3f,
    color: vec3f,
}

const_assert 1 < 2;

@group(0) @binding(0) var<uniform> light: Light;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3u) {
    var total = 0.0;
    for (var i = 0; i < 4; i++) {
        if i == 2 {
            continue;
        } else if i == 3 {
            break;
        } else {
            total += light.color[i];
        }
    }
    loop {
        total -= 1.0;
        continuing {
            break if total < 0.0;
        }
    }
    switch id.x {
        case 0u, 1u: {
            discard;
        }
        default: {
            while total > 0.0 {
                total = total - 1.0;
            }
        }
    }
}

fn helper(a: f32, b: f32) -> f32 {
    return select(a, b, a > b);
}
";

fn parse(source: &str) -> TranslationUnit {
    parse_str(source).expect("the test source should parse")
}

fn printed(source: &str) -> (String, Vec<PrintedSpan>) {
    print_with_spans(&parse(source))
}

fn find<'a>(text: &str, spans: &'a [PrintedSpan], node: &str) -> &'a PrintedSpan {
    spans
        .iter()
        .find(|s| &text[s.range.clone()] == node)
        .unwrap_or_else(|| panic!("no node printed as {node:?}"))
}

#[test]
fn recording_does_not_change_the_printed_text() {
    let tu = parse(KITCHEN_SINK);
    assert_eq!(print_with_spans(&tu).0, tu.to_string());
}

#[test]
fn blocks_are_indented_and_end_without_a_trailing_newline() {
    let (text, _) = printed("fn f() { var a = 1; { a = 2; } }");
    assert_eq!(
        text,
        "fn f() {\n    var a = 1;\n    {\n        a = 2;\n    }\n}\n"
    );
}

#[test]
fn empty_blocks_print_a_blank_line() {
    let (text, _) = printed("fn f() {}");
    assert_eq!(text, "fn f() {\n\n}\n");
}

#[test]
fn blank_lines_in_nested_blocks_keep_their_indentation() {
    let (text, _) = printed("fn f() { if true { } }");
    assert_eq!(text, "fn f() {\n    if true {\n    \n    }\n}\n");
}

#[test]
fn for_headers_drop_the_semicolons_of_their_statements() {
    let (text, spans) = printed("fn f() { for (var i = 0; i < 4; i++) { } }");
    assert!(text.contains("for (var i = 0; i < 4; i++) {"), "{text}");
    assert_eq!(find(&text, &spans, "var i = 0").depth, 2);
    assert_eq!(find(&text, &spans, "i++").depth, 2);
}

#[test]
fn spans_cover_exactly_the_printed_nodes() {
    let source = "fn f(a: i32) -> i32 {\n  let b =   a + 1;\n\n  return b;\n}";
    let (text, spans) = printed(source);
    let function = "fn f(a: i32) -> i32 {\n    let b = a + 1;\n    return b;\n}";
    assert_eq!(text, format!("{function}\n"));

    assert_eq!(find(&text, &spans, function).depth, 0);
    assert_eq!(find(&text, &spans, "let b = a + 1;").depth, 1);
    assert_eq!(find(&text, &spans, "return b;").depth, 1);
    assert_eq!(find(&text, &spans, "a + 1").depth, 2);
}

#[test]
fn spans_are_the_ones_of_the_parsed_source() {
    let source = "fn f(a: i32) -> i32 {\n  let b =   a + 1;\n\n  return b;\n}";
    let (text, spans) = printed(source);
    for (node, original) in [
        ("let b = a + 1;", "let b =   a + 1;"),
        ("return b;", "return b;"),
        ("a + 1", "a + 1"),
    ] {
        let span = find(&text, &spans, node).source_span;
        assert_eq!(&source[span.range()], original);
    }
}

#[test]
fn nodes_are_recorded_after_their_children() {
    let (text, spans) = printed("fn f() { return 1 + 2; }");
    let position = |node: &str| {
        let wanted = find(&text, &spans, node);
        spans.iter().position(|s| s == wanted).unwrap()
    };
    assert!(position("1") < position("1 + 2"));
    assert!(position("1 + 2") < position("return 1 + 2;"));
}

#[test]
fn attributes_and_struct_members_are_recorded() {
    let (text, spans) = printed("struct S { @size(16) a: f32, b: f32 }");
    assert_eq!(find(&text, &spans, "@size(16)").depth, 2);
    assert_eq!(find(&text, &spans, "@size(16)\n    a: f32").depth, 1);
    assert_eq!(find(&text, &spans, "b: f32").depth, 1);
}

#[test]
fn every_recorded_range_is_inside_the_text_and_trimmed() {
    let (text, spans) = printed(KITCHEN_SINK);
    assert_eq!(spans.iter().filter(|s| s.depth == 0).count(), 6);
    for span in spans {
        let node = text.get(span.range.clone()).expect("range is in the text");
        assert!(!node.is_empty());
        assert_eq!(node, node.trim(), "{:?}", span.range);
    }
}

#[test]
fn nodes_compose_with_other_pretty_documents() {
    let tu = parse("fn f() { }");
    let arena = Alloc::new();
    let doc = arena
        .text("// generated")
        .append(arena.hardline())
        .append(&tu);
    let mut text = String::new();
    doc.into_doc().render_fmt(usize::MAX, &mut text).unwrap();
    assert_eq!(text, format!("// generated\n{tu}"));
}
