use wesl::{Feature, Features, pass::condcomp, syntax::TranslationUnit};

/// Runs conditional translation on `source` with `X` set to `x`, and returns the result.
fn translate(source: &str, x: Feature) -> String {
    let mut module: TranslationUnit = source.parse().unwrap();
    let mut features = Features::default();
    features.set("X", x);
    condcomp(&mut module, &features).unwrap();
    module.to_string()
}

fn normalized(source: &str) -> String {
    source.parse::<TranslationUnit>().unwrap().to_string()
}

#[test]
fn keep_preserves_conditional_compound_statement() {
    let source = "fn f() { var a = 0; @if(X) { a = 1; } @else { a = 2; } }";
    assert_eq!(translate(source, Feature::Keep), normalized(source));
}

#[test]
fn keep_preserves_conditional_global_compound() {
    let source = "@if(X) { const a = 1; const b = 2; } @else { const a = 3; const b = 4; }";
    assert_eq!(translate(source, Feature::Keep), normalized(source));
}

#[test]
fn resolved_conditional_compounds_are_still_flattened() {
    assert_eq!(
        translate(
            "fn f() { var a = 0; @if(X) { a = 1; } @else { a = 2; } }",
            Feature::Enable
        ),
        normalized("fn f() { var a = 0; a = 1; }")
    );
    assert_eq!(
        translate(
            "fn f() { var a = 0; @if(X) { a = 1; } @else { a = 2; } }",
            Feature::Disable
        ),
        normalized("fn f() { var a = 0; a = 2; }")
    );
    assert_eq!(
        translate(
            "@if(X) { const a = 1; } @else { const a = 3; }",
            Feature::Enable
        ),
        normalized("const a = 1;")
    );
}
