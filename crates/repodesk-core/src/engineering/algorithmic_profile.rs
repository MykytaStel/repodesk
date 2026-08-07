//! Deterministic Rust Algorithmic Profile v0.
//!
//! This analyzer reports structural complexity hints from Rust AST evidence. It
//! deliberately does not claim proof of asymptotic complexity: type resolution,
//! loop bounds, callee bodies, iterator receiver types, and input-size relations
//! are not available in this layer. Unsupported certainty becomes `unknown`.

use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use syn::visit::{self, Visit};
use syn::{
    BinOp, Expr, ExprBinary, ExprCall, ExprClosure, ExprForLoop, ExprIf, ExprLoop, ExprMatch,
    ExprMethodCall, ExprWhile, ImplItem, ItemFn, ItemImpl, ItemTrait, Macro, Stmt, TraitItem, Type,
};

pub const MAX_ALGORITHM_SOURCE_BYTES: u64 = 1_000_000;

#[derive(Debug, thiserror::Error)]
pub enum AlgorithmicProfileError {
    #[error("algorithmic profile path must be a repository-relative Rust file: {0}")]
    InvalidPath(String),
    #[error("algorithmic profile path is blocked by RepoDesk security: {0}")]
    BlockedPath(String),
    #[error("algorithmic profile path escapes the active project: {0}")]
    OutsideProject(String),
    #[error("algorithmic profile source is too large ({bytes} bytes; max {max}): {path}")]
    TooLarge { path: String, bytes: u64, max: u64 },
    #[error("failed to read algorithmic profile source {path}: {message}")]
    Read { path: String, message: String },
    #[error("failed to parse Rust source {path}: {message}")]
    Parse { path: String, message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlgorithmicSymbolKind {
    Function,
    Method,
    TraitDefaultMethod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComplexityClass {
    Constant,
    Logarithmic,
    Linear,
    Linearithmic,
    Quadratic,
    Polynomial,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComplexityHint {
    pub class: ComplexityClass,
    pub notation: String,
}

impl ComplexityHint {
    fn new(class: ComplexityClass, notation: impl Into<String>) -> Self {
        Self {
            class,
            notation: notation.into(),
        }
    }

    fn constant() -> Self {
        Self::new(ComplexityClass::Constant, "O(1)")
    }

    fn logarithmic() -> Self {
        Self::new(ComplexityClass::Logarithmic, "O(log n)")
    }

    fn linear() -> Self {
        Self::new(ComplexityClass::Linear, "O(n)")
    }

    fn linearithmic() -> Self {
        Self::new(ComplexityClass::Linearithmic, "O(n log n)")
    }

    fn quadratic() -> Self {
        Self::new(ComplexityClass::Quadratic, "O(n^2)")
    }

    fn polynomial(degree: usize) -> Self {
        Self::new(ComplexityClass::Polynomial, format!("O(n^{degree})"))
    }

    fn unknown() -> Self {
        Self::new(ComplexityClass::Unknown, "unknown")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisConfidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlgorithmicEvidenceKind {
    ExplicitLoop,
    NestedLoop,
    IteratorScan,
    PotentialNestedScan,
    Sort,
    LogarithmicSearch,
    Recursion,
    Allocation,
    CollectionGrowth,
    Branching,
    UnresolvedCall,
    OpaqueMacro,
    Closure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlgorithmicEvidence {
    pub kind: AlgorithmicEvidenceKind,
    pub count: usize,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AlgorithmicSignals {
    pub statement_count: usize,
    pub branch_points: usize,
    pub loop_count: usize,
    pub max_loop_nesting: usize,
    pub iterator_scan_calls: usize,
    pub scan_calls_inside_loop: usize,
    pub sort_calls: usize,
    pub sort_calls_inside_loop: usize,
    pub logarithmic_search_calls: usize,
    pub recursive_calls: usize,
    pub allocation_like_calls: usize,
    pub collection_growth_calls: usize,
    pub collection_growth_calls_inside_loop: usize,
    pub unresolved_calls: usize,
    pub opaque_macros: usize,
    pub closures: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlgorithmicProfile {
    pub file: String,
    pub symbol: String,
    pub kind: AlgorithmicSymbolKind,
    pub time_complexity_hint: ComplexityHint,
    pub space_complexity_hint: ComplexityHint,
    pub confidence: AnalysisConfidence,
    pub signals: AlgorithmicSignals,
    pub evidence: Vec<AlgorithmicEvidence>,
    pub assumptions: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlgorithmicProfileReport {
    pub language: String,
    pub file: String,
    pub functions_analyzed: usize,
    pub profiles: Vec<AlgorithmicProfile>,
    pub limitations: Vec<String>,
}

pub fn analyze_rust_source(
    file: impl Into<String>,
    source: &str,
) -> Result<AlgorithmicProfileReport, AlgorithmicProfileError> {
    let file = file.into();
    let syntax = syn::parse_file(source).map_err(|error| AlgorithmicProfileError::Parse {
        path: file.clone(),
        message: error.to_string(),
    })?;

    let mut collector = ProfileCollector {
        file: file.clone(),
        profiles: Vec::new(),
    };
    collector.visit_file(&syntax);

    Ok(AlgorithmicProfileReport {
        language: "rust".to_string(),
        file,
        functions_analyzed: collector.profiles.len(),
        profiles: collector.profiles,
        limitations: vec![
            "Complexity values are structural hints, not proofs of asymptotic behavior.".to_string(),
            "v0 does not resolve callee bodies, receiver types, loop bounds, or whether independent inputs have the same size n.".to_string(),
            "Macro-expanded code and closure bodies are not analyzed in v0.".to_string(),
            "Source-line LOC is intentionally omitted until stable span locations are part of the analysis boundary; statement_count and branch_points are structural proxies.".to_string(),
        ],
    })
}

pub fn analyze_rust_file(
    project_root: &Path,
    relative_path: &str,
) -> Result<AlgorithmicProfileReport, AlgorithmicProfileError> {
    let relative = validate_rust_relative_path(relative_path)?;

    if let Some(reason) = crate::security::is_blocked_path(relative_path) {
        return Err(AlgorithmicProfileError::BlockedPath(format!(
            "{relative_path}: {reason}"
        )));
    }

    let canonical_root = fs::canonicalize(project_root).map_err(|error| {
        AlgorithmicProfileError::Read {
            path: project_root.display().to_string(),
            message: error.to_string(),
        }
    })?;
    let candidate = canonical_root.join(&relative);
    let canonical_file = fs::canonicalize(&candidate).map_err(|error| AlgorithmicProfileError::Read {
        path: relative_path.to_string(),
        message: error.to_string(),
    })?;

    if !canonical_file.starts_with(&canonical_root) {
        return Err(AlgorithmicProfileError::OutsideProject(
            relative_path.to_string(),
        ));
    }

    let metadata = fs::metadata(&canonical_file).map_err(|error| AlgorithmicProfileError::Read {
        path: relative_path.to_string(),
        message: error.to_string(),
    })?;
    if !metadata.is_file() {
        return Err(AlgorithmicProfileError::InvalidPath(
            relative_path.to_string(),
        ));
    }
    if metadata.len() > MAX_ALGORITHM_SOURCE_BYTES {
        return Err(AlgorithmicProfileError::TooLarge {
            path: relative_path.to_string(),
            bytes: metadata.len(),
            max: MAX_ALGORITHM_SOURCE_BYTES,
        });
    }

    let source = fs::read_to_string(&canonical_file).map_err(|error| AlgorithmicProfileError::Read {
        path: relative_path.to_string(),
        message: error.to_string(),
    })?;

    analyze_rust_source(relative_path.to_string(), &source)
}

fn validate_rust_relative_path(value: &str) -> Result<PathBuf, AlgorithmicProfileError> {
    let path = Path::new(value);
    if value.trim().is_empty() || path.is_absolute() || path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
        return Err(AlgorithmicProfileError::InvalidPath(value.to_string()));
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(AlgorithmicProfileError::InvalidPath(value.to_string()));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(AlgorithmicProfileError::InvalidPath(value.to_string()));
    }

    Ok(normalized)
}

struct ProfileCollector {
    file: String,
    profiles: Vec<AlgorithmicProfile>,
}

impl<'ast> Visit<'ast> for ProfileCollector {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        self.profiles.push(analyze_function(
            &self.file,
            node.sig.ident.to_string(),
            AlgorithmicSymbolKind::Function,
            &node.block,
        ));
        // Deliberately do not descend into the function body here. The dedicated
        // body visitor owns its signals and avoids double-counting nested items.
    }

    fn visit_item_impl(&mut self, node: &'ast ItemImpl) {
        let owner = type_label(&node.self_ty);
        for item in &node.items {
            if let ImplItem::Fn(method) = item {
                self.profiles.push(analyze_function(
                    &self.file,
                    format!("{owner}::{}", method.sig.ident),
                    AlgorithmicSymbolKind::Method,
                    &method.block,
                ));
            }
        }
    }

    fn visit_item_trait(&mut self, node: &'ast ItemTrait) {
        for item in &node.items {
            if let TraitItem::Fn(method) = item
                && let Some(default) = &method.default
            {
                self.profiles.push(analyze_function(
                    &self.file,
                    format!("{}::{}", node.ident, method.sig.ident),
                    AlgorithmicSymbolKind::TraitDefaultMethod,
                    default,
                ));
            }
        }
    }
}

fn type_label(ty: &Type) -> String {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
            .unwrap_or_else(|| "impl".to_string()),
        Type::Reference(reference) => type_label(&reference.elem),
        _ => "impl".to_string(),
    }
}

fn analyze_function(
    file: &str,
    symbol: String,
    kind: AlgorithmicSymbolKind,
    block: &syn::Block,
) -> AlgorithmicProfile {
    let function_name = symbol
        .rsplit("::")
        .next()
        .unwrap_or(symbol.as_str())
        .to_string();
    let mut visitor = FunctionBodyVisitor::new(function_name);
    visitor.visit_block(block);

    let signals = visitor.signals;
    let (time_complexity_hint, time_confidence, mut assumptions, mut warnings) =
        infer_time_complexity(&signals);
    let (space_complexity_hint, space_confidence, space_assumptions, space_warnings) =
        infer_space_complexity(&signals);
    assumptions.extend(space_assumptions);
    warnings.extend(space_warnings);
    assumptions.sort();
    assumptions.dedup();
    warnings.sort();
    warnings.dedup();

    AlgorithmicProfile {
        file: file.to_string(),
        symbol,
        kind,
        time_complexity_hint,
        space_complexity_hint,
        confidence: time_confidence.min(space_confidence),
        evidence: build_evidence(&signals),
        signals,
        assumptions,
        warnings,
    }
}

struct FunctionBodyVisitor {
    function_name: String,
    loop_depth: usize,
    signals: AlgorithmicSignals,
}

impl FunctionBodyVisitor {
    fn new(function_name: String) -> Self {
        Self {
            function_name,
            loop_depth: 0,
            signals: AlgorithmicSignals::default(),
        }
    }

    fn enter_loop(&mut self) {
        self.loop_depth = self.loop_depth.saturating_add(1);
        self.signals.loop_count = self.signals.loop_count.saturating_add(1);
        self.signals.branch_points = self.signals.branch_points.saturating_add(1);
        self.signals.max_loop_nesting = self.signals.max_loop_nesting.max(self.loop_depth);
    }

    fn exit_loop(&mut self) {
        self.loop_depth = self.loop_depth.saturating_sub(1);
    }
}

impl<'ast> Visit<'ast> for FunctionBodyVisitor {
    fn visit_stmt(&mut self, node: &'ast Stmt) {
        self.signals.statement_count = self.signals.statement_count.saturating_add(1);
        visit::visit_stmt(self, node);
    }

    fn visit_expr_for_loop(&mut self, node: &'ast ExprForLoop) {
        self.enter_loop();
        visit::visit_expr_for_loop(self, node);
        self.exit_loop();
    }

    fn visit_expr_while(&mut self, node: &'ast ExprWhile) {
        self.enter_loop();
        visit::visit_expr_while(self, node);
        self.exit_loop();
    }

    fn visit_expr_loop(&mut self, node: &'ast ExprLoop) {
        self.enter_loop();
        visit::visit_expr_loop(self, node);
        self.exit_loop();
    }

    fn visit_expr_if(&mut self, node: &'ast ExprIf) {
        self.signals.branch_points = self.signals.branch_points.saturating_add(1);
        visit::visit_expr_if(self, node);
    }

    fn visit_expr_match(&mut self, node: &'ast ExprMatch) {
        self.signals.branch_points = self
            .signals
            .branch_points
            .saturating_add(node.arms.len().saturating_sub(1));
        visit::visit_expr_match(self, node);
    }

    fn visit_expr_binary(&mut self, node: &'ast ExprBinary) {
        if matches!(node.op, BinOp::And(_) | BinOp::Or(_)) {
            self.signals.branch_points = self.signals.branch_points.saturating_add(1);
        }
        visit::visit_expr_binary(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        let method = node.method.to_string();
        let mut recognized = false;

        if SORT_METHODS.contains(&method.as_str()) {
            recognized = true;
            self.signals.sort_calls = self.signals.sort_calls.saturating_add(1);
            if self.loop_depth > 0 {
                self.signals.sort_calls_inside_loop =
                    self.signals.sort_calls_inside_loop.saturating_add(1);
            }
        }
        if LOGARITHMIC_SEARCH_METHODS.contains(&method.as_str()) {
            recognized = true;
            self.signals.logarithmic_search_calls =
                self.signals.logarithmic_search_calls.saturating_add(1);
        }
        if ITERATOR_SCAN_METHODS.contains(&method.as_str()) {
            recognized = true;
            self.signals.iterator_scan_calls =
                self.signals.iterator_scan_calls.saturating_add(1);
            if self.loop_depth > 0 {
                self.signals.scan_calls_inside_loop =
                    self.signals.scan_calls_inside_loop.saturating_add(1);
            }
        }
        if ALLOCATING_METHODS.contains(&method.as_str()) {
            recognized = true;
            self.signals.allocation_like_calls =
                self.signals.allocation_like_calls.saturating_add(1);
        }
        if COLLECTION_GROWTH_METHODS.contains(&method.as_str()) {
            recognized = true;
            self.signals.collection_growth_calls =
                self.signals.collection_growth_calls.saturating_add(1);
            if self.loop_depth > 0 {
                self.signals.collection_growth_calls_inside_loop = self
                    .signals
                    .collection_growth_calls_inside_loop
                    .saturating_add(1);
            }
        }
        if CONSTANTISH_METHODS.contains(&method.as_str()) {
            recognized = true;
        }

        if method == self.function_name && is_self_receiver(&node.receiver) {
            recognized = true;
            self.signals.recursive_calls = self.signals.recursive_calls.saturating_add(1);
        }

        if !recognized {
            self.signals.unresolved_calls = self.signals.unresolved_calls.saturating_add(1);
        }

        visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        let path = expression_path(&node.func);
        let recursive = path
            .as_ref()
            .and_then(|segments| segments.last())
            .is_some_and(|last| last == &self.function_name);

        if recursive {
            self.signals.recursive_calls = self.signals.recursive_calls.saturating_add(1);
        } else if path.as_ref().is_some_and(|segments| is_allocation_function(segments)) {
            self.signals.allocation_like_calls =
                self.signals.allocation_like_calls.saturating_add(1);
        } else {
            self.signals.unresolved_calls = self.signals.unresolved_calls.saturating_add(1);
        }

        visit::visit_expr_call(self, node);
    }

    fn visit_macro(&mut self, node: &'ast Macro) {
        let name = node
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
            .unwrap_or_default();
        if ALLOCATING_MACROS.contains(&name.as_str()) {
            self.signals.allocation_like_calls =
                self.signals.allocation_like_calls.saturating_add(1);
        } else {
            self.signals.opaque_macros = self.signals.opaque_macros.saturating_add(1);
        }
        // Macro expansion is intentionally opaque in v0.
    }

    fn visit_expr_closure(&mut self, _node: &'ast ExprClosure) {
        self.signals.closures = self.signals.closures.saturating_add(1);
        // Closure bodies are separate callable units and are intentionally not
        // folded into the enclosing function's complexity in v0.
    }

    fn visit_item_fn(&mut self, _node: &'ast ItemFn) {
        // Local function items are not executed merely because they are declared.
    }
}

const SORT_METHODS: &[&str] = &["sort", "sort_by", "sort_by_key", "sort_unstable", "sort_unstable_by", "sort_unstable_by_key"];
const LOGARITHMIC_SEARCH_METHODS: &[&str] = &["binary_search", "binary_search_by", "binary_search_by_key", "partition_point"];
const ITERATOR_SCAN_METHODS: &[&str] = &[
    "all", "any", "collect", "contains", "count", "find", "find_map", "fold", "for_each",
    "max", "max_by", "max_by_key", "min", "min_by", "min_by_key", "position", "product",
    "rfind", "rposition", "sum",
];
const ALLOCATING_METHODS: &[&str] = &["collect", "to_owned", "to_string", "to_vec"];
const COLLECTION_GROWTH_METHODS: &[&str] = &["extend", "extend_from_slice", "insert", "push", "push_str"];
const CONSTANTISH_METHODS: &[&str] = &[
    "as_deref", "as_mut", "as_ref", "borrow", "capacity", "clone", "copied", "expect",
    "first", "get", "get_mut", "is_empty", "is_none", "is_ok", "is_some", "last", "len",
    "ok", "unwrap", "unwrap_or", "unwrap_or_default",
];
const ALLOCATING_MACROS: &[&str] = &["format", "vec"];

fn expression_path(expr: &Expr) -> Option<Vec<String>> {
    match expr {
        Expr::Path(path) => Some(
            path.path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect(),
        ),
        _ => None,
    }
}

fn is_allocation_function(segments: &[String]) -> bool {
    let Some(last) = segments.last().map(String::as_str) else {
        return false;
    };
    let owner = segments
        .iter()
        .rev()
        .nth(1)
        .map(String::as_str)
        .unwrap_or_default();

    matches!(
        (owner, last),
        ("Box", "new")
            | ("String", "new")
            | ("String", "with_capacity")
            | ("Vec", "new")
            | ("Vec", "with_capacity")
            | ("VecDeque", "new")
            | ("HashMap", "new")
            | ("HashSet", "new")
            | ("BTreeMap", "new")
            | ("BTreeSet", "new")
    )
}

fn is_self_receiver(expr: &Expr) -> bool {
    expression_path(expr)
        .and_then(|segments| segments.last().cloned())
        .is_some_and(|segment| segment == "self")
}

fn infer_time_complexity(
    signals: &AlgorithmicSignals,
) -> (ComplexityHint, AnalysisConfidence, Vec<String>, Vec<String>) {
    let mut assumptions = Vec::new();
    let mut warnings = Vec::new();
    let opaque = signals.unresolved_calls > 0 || signals.opaque_macros > 0 || signals.closures > 0;

    if signals.recursive_calls > 0 {
        warnings.push("Explicit recursion detected, but v0 cannot prove the recurrence relation or termination behavior.".to_string());
        return (ComplexityHint::unknown(), AnalysisConfidence::Low, assumptions, warnings);
    }

    if signals.sort_calls_inside_loop > 0 {
        warnings.push("A sort-like call occurs inside a loop; v0 does not collapse this into an unsupported simple Big-O class.".to_string());
        return (ComplexityHint::unknown(), AnalysisConfidence::Low, assumptions, warnings);
    }

    if signals.scan_calls_inside_loop > 0 {
        warnings.push("A scan-like method occurs inside a loop. Receiver types are unresolved, so a nested traversal cost cannot be proven.".to_string());
        return (ComplexityHint::unknown(), AnalysisConfidence::Low, assumptions, warnings);
    }

    let (hint, mut confidence) = if signals.max_loop_nesting >= 3 {
        assumptions.push("Nested explicit loops are assumed to scale with comparable input size n.".to_string());
        (
            ComplexityHint::polynomial(signals.max_loop_nesting),
            AnalysisConfidence::Medium,
        )
    } else if signals.max_loop_nesting == 2 {
        assumptions.push("Both explicit loop bounds are assumed to scale with comparable input size n.".to_string());
        (ComplexityHint::quadratic(), AnalysisConfidence::Medium)
    } else if signals.sort_calls > 0 {
        assumptions.push("sort-like methods are assumed to have standard-library comparison-sort behavior on input-sized collections.".to_string());
        (ComplexityHint::linearithmic(), AnalysisConfidence::Medium)
    } else if signals.max_loop_nesting == 1 || signals.iterator_scan_calls > 0 {
        assumptions.push("The explicit traversal or scan-like operation is assumed to scale with input size n.".to_string());
        (ComplexityHint::linear(), AnalysisConfidence::Medium)
    } else if signals.logarithmic_search_calls > 0 {
        assumptions.push("binary-search-like methods are assumed to operate on an input-sized ordered collection.".to_string());
        (ComplexityHint::logarithmic(), AnalysisConfidence::Medium)
    } else if opaque {
        warnings.push("No explicit scaling structure was proven and unresolved calls/macros/closures may hide work.".to_string());
        return (ComplexityHint::unknown(), AnalysisConfidence::Low, assumptions, warnings);
    } else {
        (ComplexityHint::constant(), AnalysisConfidence::High)
    };

    if opaque {
        confidence = AnalysisConfidence::Low;
        assumptions.push("Unresolved callees, macro expansion, and closure bodies are assumed not to dominate the reported structural hint.".to_string());
    }

    (hint, confidence, assumptions, warnings)
}

fn infer_space_complexity(
    signals: &AlgorithmicSignals,
) -> (ComplexityHint, AnalysisConfidence, Vec<String>, Vec<String>) {
    let mut assumptions = Vec::new();
    let mut warnings = Vec::new();
    let opaque = signals.unresolved_calls > 0 || signals.opaque_macros > 0 || signals.closures > 0;

    if signals.collection_growth_calls_inside_loop > 0 {
        assumptions.push("Collection growth inside a traversal is assumed to retain elements proportional to input size.".to_string());
        let confidence = if opaque {
            AnalysisConfidence::Low
        } else {
            AnalysisConfidence::Medium
        };
        if opaque {
            assumptions.push("Unresolved callees/macros/closures are assumed not to require asymptotically more retained memory.".to_string());
        }
        return (ComplexityHint::linear(), confidence, assumptions, warnings);
    }

    if signals.iterator_scan_calls > 0 && signals.allocation_like_calls > 0 {
        assumptions.push("A collect/to_vec-like allocation is assumed to retain traversal-sized output.".to_string());
        return (ComplexityHint::linear(), AnalysisConfidence::Low, assumptions, warnings);
    }

    if signals.allocation_like_calls > 0 || signals.collection_growth_calls > 0 {
        warnings.push("Allocation-like operations are present, but v0 cannot prove whether retained allocation size scales with input.".to_string());
        return (ComplexityHint::unknown(), AnalysisConfidence::Low, assumptions, warnings);
    }

    if opaque {
        warnings.push("Unresolved calls/macros/closures may allocate memory, so constant auxiliary space cannot be claimed.".to_string());
        return (ComplexityHint::unknown(), AnalysisConfidence::Low, assumptions, warnings);
    }

    (ComplexityHint::constant(), AnalysisConfidence::High, assumptions, warnings)
}

fn build_evidence(signals: &AlgorithmicSignals) -> Vec<AlgorithmicEvidence> {
    let mut evidence = Vec::new();
    push_evidence(
        &mut evidence,
        AlgorithmicEvidenceKind::ExplicitLoop,
        signals.loop_count,
        "explicit for/while/loop expressions",
    );
    if signals.max_loop_nesting > 1 {
        evidence.push(AlgorithmicEvidence {
            kind: AlgorithmicEvidenceKind::NestedLoop,
            count: signals.max_loop_nesting,
            detail: format!("maximum explicit loop nesting depth is {}", signals.max_loop_nesting),
        });
    }
    push_evidence(
        &mut evidence,
        AlgorithmicEvidenceKind::IteratorScan,
        signals.iterator_scan_calls,
        "scan-like terminal method calls",
    );
    push_evidence(
        &mut evidence,
        AlgorithmicEvidenceKind::PotentialNestedScan,
        signals.scan_calls_inside_loop,
        "scan-like method calls observed inside explicit loops",
    );
    push_evidence(
        &mut evidence,
        AlgorithmicEvidenceKind::Sort,
        signals.sort_calls,
        "sort-like method calls",
    );
    push_evidence(
        &mut evidence,
        AlgorithmicEvidenceKind::LogarithmicSearch,
        signals.logarithmic_search_calls,
        "binary-search-like method calls",
    );
    push_evidence(
        &mut evidence,
        AlgorithmicEvidenceKind::Recursion,
        signals.recursive_calls,
        "direct self-recursive call sites",
    );
    push_evidence(
        &mut evidence,
        AlgorithmicEvidenceKind::Allocation,
        signals.allocation_like_calls,
        "allocation-like calls or macros",
    );
    push_evidence(
        &mut evidence,
        AlgorithmicEvidenceKind::CollectionGrowth,
        signals.collection_growth_calls,
        "collection growth calls",
    );
    push_evidence(
        &mut evidence,
        AlgorithmicEvidenceKind::Branching,
        signals.branch_points,
        "if/match/logical/loop structural branch points",
    );
    push_evidence(
        &mut evidence,
        AlgorithmicEvidenceKind::UnresolvedCall,
        signals.unresolved_calls,
        "calls whose complexity is not resolved in v0",
    );
    push_evidence(
        &mut evidence,
        AlgorithmicEvidenceKind::OpaqueMacro,
        signals.opaque_macros,
        "macro invocations not expanded by v0",
    );
    push_evidence(
        &mut evidence,
        AlgorithmicEvidenceKind::Closure,
        signals.closures,
        "closure bodies excluded from enclosing-function analysis",
    );
    evidence
}

fn push_evidence(
    output: &mut Vec<AlgorithmicEvidence>,
    kind: AlgorithmicEvidenceKind,
    count: usize,
    detail: &str,
) {
    if count > 0 {
        output.push(AlgorithmicEvidence {
            kind,
            count,
            detail: detail.to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn profile(source: &str, symbol: &str) -> AlgorithmicProfile {
        analyze_rust_source("src/lib.rs", source)
            .unwrap()
            .profiles
            .into_iter()
            .find(|profile| profile.symbol == symbol)
            .unwrap()
    }

    #[test]
    fn constant_function_is_only_claimed_without_opaque_calls() {
        let report = profile("fn answer(x: u64) -> u64 { let y = x + 1; y * 2 }", "answer");
        assert_eq!(report.time_complexity_hint.class, ComplexityClass::Constant);
        assert_eq!(report.space_complexity_hint.class, ComplexityClass::Constant);
        assert_eq!(report.confidence, AnalysisConfidence::High);
    }

    #[test]
    fn explicit_single_loop_is_linear_hint() {
        let report = profile("fn sum(xs: &[u64]) -> u64 { let mut s = 0; for x in xs { s += *x; } s }", "sum");
        assert_eq!(report.time_complexity_hint.class, ComplexityClass::Linear);
        assert_eq!(report.signals.loop_count, 1);
        assert_eq!(report.signals.max_loop_nesting, 1);
    }

    #[test]
    fn nested_explicit_loops_are_quadratic_hint() {
        let report = profile("fn pairs(xs: &[u8]) { for _a in xs { for _b in xs {} } }", "pairs");
        assert_eq!(report.time_complexity_hint.class, ComplexityClass::Quadratic);
        assert_eq!(report.signals.max_loop_nesting, 2);
    }

    #[test]
    fn sort_is_linearithmic_structural_hint() {
        let report = profile("fn order(xs: &mut [u8]) { xs.sort_unstable(); }", "order");
        assert_eq!(report.time_complexity_hint.class, ComplexityClass::Linearithmic);
        assert_eq!(report.signals.sort_calls, 1);
    }

    #[test]
    fn recursion_prefers_unknown_over_fake_big_o() {
        let report = profile("fn recurse(n: u64) -> u64 { if n == 0 { 0 } else { recurse(n - 1) } }", "recurse");
        assert_eq!(report.time_complexity_hint.class, ComplexityClass::Unknown);
        assert_eq!(report.signals.recursive_calls, 1);
        assert_eq!(report.confidence, AnalysisConfidence::Low);
    }

    #[test]
    fn potential_scan_inside_loop_is_unknown_without_type_resolution() {
        let report = profile("fn overlap(xs: &[u8], ys: &[u8]) -> bool { for x in xs { if ys.contains(x) { return true; } } false }", "overlap");
        assert_eq!(report.time_complexity_hint.class, ComplexityClass::Unknown);
        assert_eq!(report.signals.scan_calls_inside_loop, 1);
        assert!(!report.warnings.is_empty());
    }

    #[test]
    fn collection_growth_in_loop_is_linear_space_hint() {
        let report = profile("fn copy(xs: &[u8]) -> Vec<u8> { let mut out = Vec::new(); for x in xs { out.push(*x); } out }", "copy");
        assert_eq!(report.space_complexity_hint.class, ComplexityClass::Linear);
        assert_eq!(report.signals.collection_growth_calls_inside_loop, 1);
    }

    #[test]
    fn impl_and_trait_default_symbols_are_named() {
        let report = analyze_rust_source(
            "src/lib.rs",
            "struct Store; impl Store { fn len_twice(&self, xs: &[u8]) -> usize { for _ in xs {} xs.len() } } trait Scan { fn scan(&self, xs: &[u8]) { for _ in xs {} } }",
        )
        .unwrap();

        assert!(report.profiles.iter().any(|profile| profile.symbol == "Store::len_twice"));
        assert!(report.profiles.iter().any(|profile| profile.symbol == "Scan::scan"));
    }

    #[test]
    fn invalid_rust_returns_parse_error() {
        let error = analyze_rust_source("src/lib.rs", "fn broken(").unwrap_err();
        assert!(matches!(error, AlgorithmicProfileError::Parse { .. }));
    }

    #[test]
    fn file_analysis_guards_path_and_project_boundary() {
        let root = tempdir().unwrap();
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::write(root.path().join("src/lib.rs"), "fn ok() {}\n").unwrap();

        let report = analyze_rust_file(root.path(), "src/lib.rs").unwrap();
        assert_eq!(report.functions_analyzed, 1);
        assert!(analyze_rust_file(root.path(), "../escape.rs").is_err());
        assert!(analyze_rust_file(root.path(), "src/lib.ts").is_err());
    }
}
