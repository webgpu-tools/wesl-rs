//! File containing the serde data structures for JSON test files.
//! See schemas: <https://github.com/webgpu-tools/wesl-testsuite/blob/main/src/TestSchema.ts>

use std::{
    collections::HashMap,
    fmt::{self},
};

use regex::Regex;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WgslTestSrc {
    pub name: String,
    pub wesl_src: HashMap<String, String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub requires: Vec<String>,
    #[allow(unused)]
    #[serde(default)]
    pub expected_wgsl: Option<String>,
    #[serde(default)]
    pub underscore_wgsl: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WgslBulkTest {
    pub name: String,
    pub base_dir: String,
    pub git: Option<WgslGitSrc>,
    pub include: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WgslGitSrc {
    pub url: String,
    pub revision: String,
}

impl WgslTestSrc {
    pub fn normalize(&mut self) {
        self.wesl_src.values_mut().for_each(|src| {
            *src = normalize_wgsl(src);
        });
        if let Some(expected_wgsl) = &self.expected_wgsl {
            self.expected_wgsl = Some(normalize_wgsl(expected_wgsl))
        }
        if let Some(underscore_wgsl) = &self.underscore_wgsl {
            self.underscore_wgsl = Some(normalize_wgsl(underscore_wgsl))
        }
    }
}

impl fmt::Display for WgslTestSrc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Deserialize)]
pub struct ParsingTest {
    pub src: String,
    #[serde(default)]
    pub fails: bool,
}

impl ParsingTest {
    pub fn normalize(&mut self) {
        self.src = normalize_wgsl(&self.src)
    }
}

impl fmt::Display for ParsingTest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}`", normalize_wgsl(&self.src))
    }
}

fn normalize_wgsl(wgsl: &str) -> String {
    let re = Regex::new(r"\s+").unwrap();
    re.replace_all(wgsl, " ").trim().to_string()
}

#[allow(unused)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkTest {
    pub name: String,
    pub base_dir: String,
    pub exclude: Option<Vec<String>>,
    pub include: Option<Vec<String>>,
    pub glob_include: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub struct Test {
    pub name: String,
    pub desc: String,
    #[serde(flatten)]
    pub kind: TestKind,
    pub code: String,
    pub expect: Expectation,
    pub note: Option<String>,
    pub skip: Option<bool>,
    pub issue: Option<String>,
}

impl fmt::Display for Test {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SyntaxKind {
    Declaration,
    Statement,
    Expression,
}

#[derive(PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "kind")]
pub enum TestKind {
    Syntax {
        syntax: SyntaxKind,
    },
    Eval {
        eval: String,
        result: Option<String>, // must be None when expect is Fail, must be Some when expect is Pass
    },
    Context {
        #[serde(default)]
        lower: bool,
    },
}

impl fmt::Display for TestKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestKind::Syntax { .. } => f.write_str("Syntax"),
            TestKind::Eval { .. } => f.write_str("Eval"),
            TestKind::Context { .. } => f.write_str("Context"),
        }
    }
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[derive(Clone, Copy)]
pub enum Expectation {
    Pass,
    Fail,
}

impl From<bool> for Expectation {
    fn from(value: bool) -> Self {
        if value {
            Self::Pass
        } else {
            Self::Fail
        }
    }
}

impl fmt::Display for Expectation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expectation::Pass => f.write_str("Pass"),
            Expectation::Fail => f.write_str("Fail"),
        }
    }
}
