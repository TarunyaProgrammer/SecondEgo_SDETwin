use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde::{Deserialize, Serialize};
use tree_sitter::{Node, Parser};
use walkdir::{DirEntry, WalkDir};

const IGNORED_DIRECTORIES: &[&str] = &[
    ".git",
    ".venv",
    "node_modules",
    "__pycache__",
    "dist",
    "build",
    "target",
];
const MANIFESTS: &[&str] = &[
    "pyproject.toml",
    "package.json",
    "pytest.ini",
    "tox.ini",
    "setup.cfg",
    "Cargo.toml",
    "tsconfig.json",
    "vite.config.ts",
];
const MAX_SOURCE_BYTES: u64 = 512 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositorySnapshot {
    pub root: String,
    pub files: Vec<String>,
    pub manifests: Vec<String>,
    pub test_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: String,
    pub path: String,
    pub line_start: usize,
    pub line_end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportEdge {
    pub source_path: String,
    pub module: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParserFailure {
    pub path: String,
    pub message: String,
    pub line: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestNode {
    pub path: String,
    pub name: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestLink {
    pub test_path: String,
    pub target_path: String,
    pub reason: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RankedFile {
    pub path: String,
    pub score: i32,
    pub reasons: Vec<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepositoryIndex {
    pub snapshot: RepositorySnapshot,
    pub symbols: Vec<Symbol>,
    pub imports: Vec<ImportEdge>,
    /// Bounded lexical terms from supported source files. This is not an
    /// embedding store: it remains local, rebuildable, and explainable.
    #[serde(default)]
    pub content_terms: BTreeMap<String, BTreeSet<String>>,
    pub parser_failures: Vec<ParserFailure>,
    pub tests: Vec<TestNode>,
    pub test_links: Vec<TestLink>,
}

impl RepositoryIndex {
    pub fn importers_of(&self, module: &str) -> Vec<&ImportEdge> {
        self.imports
            .iter()
            .filter(|edge| edge.module == module)
            .collect()
    }

    pub fn rank(&self, query: &str, limit: usize, failure_mode: bool) -> Vec<RankedFile> {
        const STOP_WORDS: &[&str] = &[
            "the", "in", "on", "at", "to", "for", "of", "and", "or", "is", "are",
            "was", "were", "not", "do", "does", "did", "so", "that", "this", "after",
            "before", "from", "with", "a", "an", "be", "been", "can", "could", "would",
            "should", "have", "has", "had", "once", "again", "st", "nd", "rd", "th",
        ];
        let raw_terms = query_terms(query);
        let terms: Vec<String> = raw_terms
            .into_iter()
            .filter(|t| t.len() >= 3 && !STOP_WORDS.contains(&t.as_str()))
            .collect();
        let mut scores: BTreeMap<String, i32> = BTreeMap::new();
        let mut reasons: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let symbols_by_path =
            self.symbols
                .iter()
                .fold(BTreeMap::<String, Vec<String>>::new(), |mut map, symbol| {
                    map.entry(symbol.path.clone())
                        .or_default()
                        .push(symbol.name.to_lowercase());
                    map
                });

        let mut doc_freq: BTreeMap<&str, usize> = BTreeMap::new();
        for terms_set in self.content_terms.values() {
            for term in &terms {
                if terms_set.contains(term) {
                    *doc_freq.entry(term.as_str()).or_default() += 1;
                }
            }
        }

        for path in &self.snapshot.files {
            let lowered = path.to_lowercase();
            for term in &terms {
                if lowered.contains(term) {
                    add_score(&mut scores, &mut reasons, path, 3, format!("path:{term}"));
                }
                if symbols_by_path
                    .get(path)
                    .is_some_and(|symbols| symbols.iter().any(|symbol| symbol.contains(term)))
                {
                    add_score(&mut scores, &mut reasons, path, 5, format!("symbol:{term}"));
                }
                if self
                    .content_terms
                    .get(path)
                    .is_some_and(|terms| terms.contains(term))
                {
                    let df = doc_freq.get(term.as_str()).copied().unwrap_or(1);
                    let specificity_score = if df <= 2 {
                        8
                    } else if df <= 4 {
                        4
                    } else {
                        2
                    };
                    add_score(
                        &mut scores,
                        &mut reasons,
                        path,
                        specificity_score,
                        format!("content:{term}"),
                    );
                }
            }
            if self.snapshot.test_files.contains(path)
                && terms
                    .iter()
                    .any(|term| matches!(term.as_str(), "test" | "bug" | "fix" | "regression"))
            {
                add_score(&mut scores, &mut reasons, path, 1, "test-relevance".into());
            }
            if (path.ends_with("/lib.rs") || path.ends_with("/main.rs"))
                && !terms.iter().any(|t| t == "lib" || t == "main")
            {
                if let Some(score) = scores.get_mut(path) {
                    *score = score.saturating_sub(10);
                }
            }
        }

        for link in &self.test_links {
            let test_matches = terms
                .iter()
                .any(|term| link.test_path.to_lowercase().contains(term));
            if test_matches {
                add_score(
                    &mut scores,
                    &mut reasons,
                    &link.test_path,
                    8,
                    "failure-test".into(),
                );
                add_score(
                    &mut scores,
                    &mut reasons,
                    &link.target_path,
                    7,
                    "linked-from-test".into(),
                );
            }
            if failure_mode
                && terms
                    .iter()
                    .any(|term| matches!(term.as_str(), "failure" | "failed" | "error" | "assert"))
            {
                add_score(
                    &mut scores,
                    &mut reasons,
                    &link.target_path,
                    2,
                    "test-topology".into(),
                );
            }
        }

        // Multi-file dependency & blast radius propagation across import graph
        let mut module_to_file = BTreeMap::new();
        for path in &self.snapshot.files {
            for module in module_keys_for_path(path) {
                module_to_file.entry(module).or_insert_with(|| path.clone());
            }
        }

        let scored_files: Vec<(String, i32)> = scores
            .iter()
            .filter(|&(_, &score)| score >= 3)
            .map(|(path, &score)| (path.clone(), score))
            .collect();

        for (scored_path, _) in scored_files {
            for edge in &self.imports {
                let edge_target = import_targets(&edge.module, &edge.source_path)
                    .iter()
                    .find_map(|module| module_to_file.get(module))
                    .cloned();
                if edge_target.as_deref() == Some(&scored_path) && edge.source_path != scored_path {
                    add_score(
                        &mut scores,
                        &mut reasons,
                        &edge.source_path,
                        4,
                        format!("importer-of:{}", scored_path),
                    );
                } else if edge.source_path == scored_path {
                    if let Some(dep_path) = import_targets(&edge.module, &edge.source_path)
                        .iter()
                        .find_map(|module| module_to_file.get(module))
                    {
                        if dep_path != &scored_path {
                            add_score(
                                &mut scores,
                                &mut reasons,
                                dep_path,
                                3,
                                format!("dependency-of:{}", scored_path),
                            );
                        }
                    }
                }
            }
        }

        let mut ranked: Vec<RankedFile> = scores
            .into_iter()
            .filter_map(|(path, score)| {
                let file_reasons: Vec<String> = reasons.remove(&path)?.into_iter().collect();
                Some(RankedFile {
                    path,
                    score,
                    confidence: if file_reasons.iter().any(|reason| reason.contains("test")) {
                        0.95
                    } else {
                        0.6
                    },
                    reasons: file_reasons,
                })
            })
            .collect();
        ranked.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.path.cmp(&right.path))
        });
        ranked.truncate(limit);
        ranked
    }
}

pub struct RepositoryIndexer {
    root: PathBuf,
    max_files: usize,
}

impl RepositoryIndexer {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            max_files: 20_000,
        }
    }

    pub fn with_max_files(mut self, max_files: usize) -> Self {
        self.max_files = max_files;
        self
    }

    pub fn build(&self) -> std::io::Result<RepositoryIndex> {
        let snapshot = self.scan()?;
        let mut symbols = Vec::new();
        let mut imports = Vec::new();
        let mut content_terms = BTreeMap::new();
        let mut parser_failures = Vec::new();
        let mut tests = Vec::new();
        let mut parser = Parser::new();

        for path in &snapshot.files {
            let Some(language) = SourceLanguage::for_path(path) else {
                continue;
            };
            let absolute = self.root.join(path);
            let metadata = fs::metadata(&absolute)?;
            if metadata.len() > MAX_SOURCE_BYTES {
                parser_failures.push(ParserFailure {
                    path: path.clone(),
                    message: "source exceeds indexing limit".into(),
                    line: None,
                });
                continue;
            }
            let source = fs::read_to_string(&absolute)
                .map_err(|error| std::io::Error::other(format!("{path}: {error}")))?;
            content_terms.insert(path.clone(), query_terms(&source).into_iter().collect());
            if let Err(error) = language.set_parser_language(&mut parser) {
                parser_failures.push(ParserFailure {
                    path: path.clone(),
                    message: format!("tree-sitter language setup failed: {error}"),
                    line: None,
                });
                continue;
            }
            let Some(tree) = parser.parse(&source, None) else {
                parser_failures.push(ParserFailure {
                    path: path.clone(),
                    message: "tree-sitter returned no syntax tree".into(),
                    line: None,
                });
                continue;
            };
            if tree.root_node().has_error() {
                parser_failures.push(ParserFailure {
                    path: path.clone(),
                    message: "tree-sitter syntax errors present".into(),
                    line: first_error_line(tree.root_node()),
                });
            }
            collect_nodes(
                tree.root_node(),
                &source,
                path,
                language,
                &mut symbols,
                &mut imports,
                &mut tests,
                snapshot.test_files.iter().any(|test| test == path),
            );
        }

        let test_links = link_tests(&snapshot, &imports);
        symbols
            .sort_by_key(|item: &Symbol| (item.path.clone(), item.line_start, item.name.clone()));
        imports.sort_by_key(|item: &ImportEdge| {
            (item.source_path.clone(), item.line, item.module.clone())
        });
        tests.sort_by_key(|item: &TestNode| (item.path.clone(), item.line, item.name.clone()));
        parser_failures.sort_by_key(|item: &ParserFailure| item.path.clone());
        Ok(RepositoryIndex {
            snapshot,
            symbols,
            imports,
            content_terms,
            parser_failures,
            tests,
            test_links,
        })
    }

    fn scan(&self) -> std::io::Result<RepositorySnapshot> {
        let mut files = Vec::new();
        let mut manifests = Vec::new();
        let mut test_files = Vec::new();
        for entry in WalkDir::new(&self.root)
            .follow_links(false)
            .into_iter()
            .filter_entry(should_descend)
            .filter_map(Result::ok)
        {
            if files.len() >= self.max_files {
                break;
            }
            if !entry.file_type().is_file() {
                continue;
            }
            let relative = entry
                .path()
                .strip_prefix(&self.root)
                .expect("walkdir path must be under root");
            if is_sensitive_relative_path(relative) {
                continue;
            }
            let path = relative.to_string_lossy().replace('\\', "/");
            if MANIFESTS
                .iter()
                .any(|manifest| *manifest == entry.file_name().to_string_lossy())
            {
                manifests.push(path.clone());
            }
            if is_test_file(relative) {
                test_files.push(path.clone());
            }
            files.push(path);
        }
        files.sort();
        manifests.sort();
        test_files.sort();
        Ok(RepositorySnapshot {
            root: self.root.to_string_lossy().into_owned(),
            files,
            manifests,
            test_files,
        })
    }
}

fn should_descend(entry: &DirEntry) -> bool {
    !entry.file_type().is_dir()
        || entry
            .file_name()
            .to_str()
            .is_none_or(|name| !IGNORED_DIRECTORIES.contains(&name))
}

fn is_sensitive_relative_path(path: &Path) -> bool {
    path.components().any(|part| {
        let name = part.as_os_str().to_str().unwrap_or_default();
        name == ".git"
            || name == ".env"
            || name.starts_with(".env.")
            || matches!(
                name,
                "credentials.json" | "secrets.json" | "id_rsa" | "id_ed25519"
            )
    })
}

fn is_test_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_lowercase();
    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("py" | "js" | "jsx" | "ts" | "tsx" | "rs")
    ) && (name.starts_with("test_")
        || name.ends_with("_test.py")
        || name.contains(".test.")
        || name.contains(".spec.")
        || path.components().any(|part| part.as_os_str() == "tests"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceLanguage {
    Python,
    JavaScript,
    TypeScript,
    Tsx,
    Rust,
}

impl SourceLanguage {
    fn for_path(path: &str) -> Option<Self> {
        match Path::new(path).extension().and_then(|value| value.to_str()) {
            Some("py") => Some(Self::Python),
            Some("js" | "jsx" | "mjs" | "cjs") => Some(Self::JavaScript),
            Some("ts") => Some(Self::TypeScript),
            Some("tsx") => Some(Self::Tsx),
            Some("rs") => Some(Self::Rust),
            _ => None,
        }
    }

    fn set_parser_language(self, parser: &mut Parser) -> Result<(), String> {
        let result = match self {
            Self::Python => parser.set_language(&tree_sitter_python::LANGUAGE.into()),
            Self::JavaScript => parser.set_language(&tree_sitter_javascript::LANGUAGE.into()),
            Self::TypeScript => {
                parser.set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
            }
            Self::Tsx => parser.set_language(&tree_sitter_typescript::LANGUAGE_TSX.into()),
            Self::Rust => parser.set_language(&tree_sitter_rust::LANGUAGE.into()),
        };
        result.map_err(|error| error.to_string())
    }

    fn is_import_node(self, kind: &str) -> bool {
        match self {
            Self::Python => matches!(kind, "import_statement" | "import_from_statement"),
            Self::JavaScript | Self::TypeScript | Self::Tsx => {
                matches!(kind, "import_statement" | "call_expression")
            }
            Self::Rust => kind == "use_declaration",
        }
    }

    fn imports(self, raw: &str) -> Vec<String> {
        match self {
            Self::Python => parse_python_import_modules(raw),
            Self::JavaScript | Self::TypeScript | Self::Tsx => parse_javascript_import_modules(raw),
            Self::Rust => parse_rust_import_modules(raw),
        }
    }

    fn symbol_kind(self, node_kind: &str) -> Option<&'static str> {
        match self {
            Self::Python => match node_kind {
                "function_definition" => Some("function"),
                "class_definition" => Some("class"),
                _ => None,
            },
            Self::JavaScript | Self::TypeScript | Self::Tsx => match node_kind {
                "function_declaration" | "generator_function_declaration" | "method_definition" => {
                    Some("function")
                }
                "class_declaration" | "abstract_class_declaration" => Some("class"),
                "interface_declaration" => Some("interface"),
                "type_alias_declaration" => Some("type"),
                "enum_declaration" => Some("enum"),
                _ => None,
            },
            Self::Rust => match node_kind {
                "function_item" => Some("function"),
                "struct_item" => Some("struct"),
                "enum_item" => Some("enum"),
                "trait_item" => Some("trait"),
                "impl_item" => Some("impl"),
                "mod_item" => Some("module"),
                "type_item" => Some("type"),
                _ => None,
            },
        }
    }
}

fn collect_nodes(
    root: Node<'_>,
    source: &str,
    path: &str,
    language: SourceLanguage,
    symbols: &mut Vec<Symbol>,
    imports: &mut Vec<ImportEdge>,
    tests: &mut Vec<TestNode>,
    is_test_file: bool,
) {
    let mut cursor = root.walk();
    for node in root.children(&mut cursor) {
        let kind = node.kind();
        if let Some(symbol_kind) = language.symbol_kind(kind) {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = name_node
                    .utf8_text(source.as_bytes())
                    .unwrap_or("<invalid>")
                    .to_owned();
                let line = node.start_position().row + 1;
                symbols.push(Symbol {
                    name: name.clone(),
                    kind: symbol_kind.into(),
                    path: path.into(),
                    line_start: line,
                    line_end: node.end_position().row + 1,
                });
                if is_test_file && is_test_symbol(language, symbol_kind, &name) {
                    tests.push(TestNode {
                        path: path.into(),
                        name,
                        line,
                    });
                }
            }
        } else if language.is_import_node(kind) {
            let raw = node.utf8_text(source.as_bytes()).unwrap_or_default();
            for module in language.imports(raw) {
                imports.push(ImportEdge {
                    source_path: path.into(),
                    module,
                    line: node.start_position().row + 1,
                });
            }
        }
        if is_test_file
            && matches!(
                language,
                SourceLanguage::JavaScript | SourceLanguage::TypeScript | SourceLanguage::Tsx
            )
            && kind == "call_expression"
        {
            if let Some(name) =
                javascript_test_name(node.utf8_text(source.as_bytes()).unwrap_or_default())
            {
                tests.push(TestNode {
                    path: path.into(),
                    name,
                    line: node.start_position().row + 1,
                });
            }
        }
        collect_nodes(
            node,
            source,
            path,
            language,
            symbols,
            imports,
            tests,
            is_test_file,
        );
    }
}

fn is_test_symbol(language: SourceLanguage, symbol_kind: &str, name: &str) -> bool {
    symbol_kind == "function"
        && (name.starts_with("test")
            || (matches!(language, SourceLanguage::Rust) && name.contains("test")))
}

fn javascript_test_name(raw: &str) -> Option<String> {
    let trimmed = raw.trim_start();
    let prefix = ["test(", "it(", "describe("]
        .into_iter()
        .find(|prefix| trimmed.starts_with(prefix))?;
    let rest = trimmed.strip_prefix(prefix)?.trim_start();
    let quote = rest.chars().next()?;
    if !matches!(quote, '\'' | '\"' | '`') {
        return None;
    }
    rest[quote.len_utf8()..]
        .split(quote)
        .next()
        .filter(|name| !name.trim().is_empty())
        .map(str::to_owned)
}

fn parse_python_import_modules(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    if let Some(value) = trimmed.strip_prefix("import ") {
        return value
            .split(',')
            .filter_map(|item| item.trim().split(" as ").next())
            .map(str::to_owned)
            .collect();
    }
    if let Some(value) = trimmed.strip_prefix("from ") {
        return value
            .split(" import ")
            .next()
            .map(str::to_owned)
            .into_iter()
            .collect();
    }
    Vec::new()
}

fn parse_javascript_import_modules(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    let patterns = [" from ", "import ", "require("];
    let Some(candidate) = patterns.iter().find_map(|marker| {
        trimmed
            .find(marker)
            .map(|index| &trimmed[index + marker.len()..])
    }) else {
        return Vec::new();
    };
    let candidate = candidate.trim_start();
    let Some(quote) = candidate.chars().next() else {
        return Vec::new();
    };
    if !matches!(quote, '\'' | '\"' | '`') {
        return Vec::new();
    }
    candidate[quote.len_utf8()..]
        .split(quote)
        .next()
        .filter(|value| !value.is_empty())
        .map(|value| vec![value.to_owned()])
        .unwrap_or_default()
}

fn parse_rust_import_modules(raw: &str) -> Vec<String> {
    raw.trim()
        .strip_prefix("use ")
        .map(|value| {
            value
                .trim_end_matches(';')
                .split("::{")
                .next()
                .unwrap_or(value)
                .trim_end_matches("::*")
                .to_owned()
        })
        .filter(|value| !value.is_empty())
        .into_iter()
        .collect()
}

fn link_tests(snapshot: &RepositorySnapshot, imports: &[ImportEdge]) -> Vec<TestLink> {
    let mut module_paths = BTreeMap::new();
    for path in &snapshot.files {
        for module in module_keys_for_path(path) {
            module_paths.entry(module).or_insert_with(|| path.clone());
        }
    }
    let mut links = BTreeSet::new();
    for edge in imports
        .iter()
        .filter(|edge| snapshot.test_files.contains(&edge.source_path))
    {
        let target = import_targets(&edge.module, &edge.source_path)
            .iter()
            .find_map(|module| module_paths.get(module))
            .cloned()
            .or_else(|| {
                module_paths
                    .iter()
                    .find(|(module, _)| {
                        module.ends_with(&format!(".{0}", edge.module))
                            || module.ends_with(&format!("::{0}", edge.module))
                            || edge.module.ends_with(&format!(".{0}", module))
                    })
                    .map(|(_, path)| path.clone())
            });
        if let Some(target_path) = target.filter(|path| !snapshot.test_files.contains(path)) {
            links.insert((edge.source_path.clone(), target_path));
        }
    }
    links
        .into_iter()
        .map(|(test_path, target_path)| TestLink {
            test_path,
            target_path,
            reason: "test_import".into(),
            confidence: 0.95,
        })
        .collect()
}

fn module_keys_for_path(path: &str) -> Vec<String> {
    let Some(language) = SourceLanguage::for_path(path) else {
        return Vec::new();
    };
    let extension = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let stem = path
        .strip_suffix(&format!(".{extension}"))
        .unwrap_or(path)
        .trim_end_matches("/index")
        .trim_end_matches("/__init__")
        .to_owned();
    let mut keys = BTreeSet::from([stem.clone()]);
    keys.insert(stem.replace('/', "."));
    keys.insert(stem.replace('/', "::"));
    if let Some(base) = stem.rsplit('/').next() {
        keys.insert(base.to_owned());
    }
    if matches!(language, SourceLanguage::Rust) {
        let rust_stem = stem.strip_prefix("src/").unwrap_or(&stem);
        if rust_stem == "lib" || rust_stem == "main" {
            keys.insert("crate".into());
        } else {
            keys.insert(format!("crate::{}", rust_stem.replace('/', "::")));
        }
    }
    keys.into_iter().filter(|key| !key.is_empty()).collect()
}

fn import_targets(module: &str, source_path: &str) -> Vec<String> {
    let module = module
        .trim()
        .trim_matches('\'')
        .trim_matches('\"')
        .trim_end_matches(';')
        .trim_end_matches("::*");
    if module.is_empty() {
        return Vec::new();
    }
    let mut keys = BTreeSet::from([module.to_owned()]);
    let without_extension = [".py", ".js", ".jsx", ".ts", ".tsx", ".rs"]
        .into_iter()
        .find_map(|extension| module.strip_suffix(extension))
        .unwrap_or(module);
    keys.insert(without_extension.to_owned());
    if module.starts_with('.') {
        let mut segments = source_path
            .rsplit_once('/')
            .map(|(parent, _)| parent.split('/').map(str::to_owned).collect::<Vec<_>>())
            .unwrap_or_default();
        for component in without_extension.split('/') {
            match component {
                "" | "." => {}
                ".." => {
                    segments.pop();
                }
                name => segments.push(name.to_owned()),
            }
        }
        let relative = segments.join("/");
        if !relative.is_empty() {
            keys.insert(relative.clone());
            keys.insert(relative.trim_end_matches("/index").to_owned());
            if let Some(base) = relative.rsplit('/').next() {
                keys.insert(base.to_owned());
            }
        }
    }
    if let Some((first, _)) = without_extension.split_once("::") {
        keys.insert(first.to_owned());
    }
    if let Some(base) = without_extension
        .trim_start_matches("crate::")
        .split("::")
        .next()
    {
        keys.insert(base.to_owned());
    }
    if let Some(base) = without_extension.rsplit('.').next() {
        keys.insert(base.to_owned());
    }
    keys.into_iter().filter(|key| !key.is_empty()).collect()
}

fn first_error_line(root: Node<'_>) -> Option<usize> {
    if root.is_error() || root.is_missing() {
        return Some(root.start_position().row + 1);
    }
    let mut cursor = root.walk();
    root.children(&mut cursor).find_map(first_error_line)
}

fn query_terms(query: &str) -> Vec<String> {
    Regex::new(r"[A-Za-z_][A-Za-z0-9_]*")
        .expect("static regex")
        .find_iter(query)
        .map(|item| item.as_str().to_lowercase())
        .collect()
}

fn add_score(
    scores: &mut BTreeMap<String, i32>,
    reasons: &mut BTreeMap<String, BTreeSet<String>>,
    path: &str,
    score: i32,
    reason: String,
) {
    *scores.entry(path.to_owned()).or_default() += score;
    reasons.entry(path.to_owned()).or_default().insert(reason);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir_all(directory.path().join("src")).unwrap();
        fs::create_dir_all(directory.path().join("tests")).unwrap();
        fs::write(
            directory.path().join("src/app.py"),
            "def validate_token(value):\n    return value\n",
        )
        .unwrap();
        fs::write(directory.path().join("tests/test_app.py"), "from src.app import validate_token\n\ndef test_validate_token():\n    assert validate_token('ok') == 'ok'\n").unwrap();
        directory
    }

    #[test]
    fn tree_sitter_index_records_symbols_imports_tests_and_links() {
        let directory = fixture();
        let index = RepositoryIndexer::new(directory.path()).build().unwrap();
        assert!(
            index
                .symbols
                .iter()
                .any(|symbol| symbol.name == "validate_token")
        );
        assert!(index.imports.iter().any(|edge| edge.module == "src.app"));
        assert!(
            index
                .tests
                .iter()
                .any(|test| test.name == "test_validate_token")
        );
        assert!(
            index
                .test_links
                .iter()
                .any(|link| link.target_path == "src/app.py")
        );
    }

    #[test]
    fn index_supports_typescript_and_rust_symbols_imports_and_test_links() {
        let directory = fixture();
        fs::write(
            directory.path().join("src/status_cache.ts"),
            "export function invalidateProjectCache(projectId: string) { return projectId; }\n",
        )
        .unwrap();
        fs::write(
            directory.path().join("tests/status_cache.test.ts"),
            "import { invalidateProjectCache } from '../src/status_cache';\ntest('invalidates cache', () => invalidateProjectCache('p'));\n",
        )
        .unwrap();
        fs::write(
            directory.path().join("src/rust_cache.rs"),
            "pub fn invalidate_rust_cache() {}\n",
        )
        .unwrap();
        fs::write(
            directory.path().join("tests/rust_cache_test.rs"),
            "use crate::rust_cache::invalidate_rust_cache;\n#[test]\nfn test_cache() { invalidate_rust_cache(); }\n",
        )
        .unwrap();

        let index = RepositoryIndexer::new(directory.path()).build().unwrap();
        assert!(
            index
                .symbols
                .iter()
                .any(|symbol| symbol.name == "invalidateProjectCache")
        );
        assert!(
            index
                .symbols
                .iter()
                .any(|symbol| symbol.name == "invalidate_rust_cache")
        );
        assert!(index.test_links.iter().any(|link| {
            link.test_path == "tests/status_cache.test.ts"
                && link.target_path == "src/status_cache.ts"
        }));
        assert!(index.test_links.iter().any(|link| {
            link.test_path == "tests/rust_cache_test.rs" && link.target_path == "src/rust_cache.rs"
        }));
        assert!(
            index
                .rank("invalidate project cache", 8, false)
                .iter()
                .any(|item| item.path == "src/status_cache.ts")
        );
    }

    #[test]
    fn retrieval_explains_test_topology_bonus() {
        let directory = fixture();
        let index = RepositoryIndexer::new(directory.path()).build().unwrap();
        let ranked = index.rank("test validate token failure", 8, true);
        let target = ranked
            .iter()
            .find(|item| item.path == "src/app.py")
            .unwrap();
        assert!(
            target
                .reasons
                .iter()
                .any(|reason| reason == "linked-from-test")
        );
        assert!(target.confidence >= 0.95);
    }

    #[test]
    fn scanner_skips_generated_and_dependency_directories() {
        let directory = fixture();
        fs::create_dir_all(directory.path().join("target")).unwrap();
        fs::write(directory.path().join("target/generated.py"), "VALUE = 1").unwrap();
        let index = RepositoryIndexer::new(directory.path()).build().unwrap();
        assert!(
            !index
                .snapshot
                .files
                .iter()
                .any(|path| path.starts_with("target/"))
        );
    }

    #[test]
    fn retrieval_propagates_multifile_import_dependencies() {
        let directory = fixture();
        // Add a caller file that imports app.py
        fs::write(
            directory.path().join("src/caller.py"),
            "from src.app import validate_token\n\ndef run():\n    return validate_token(1)\n",
        )
        .unwrap();
        let index = RepositoryIndexer::new(directory.path()).build().unwrap();
        let ranked = index.rank("validate_token", 8, false);
        let caller = ranked.iter().find(|item| item.path == "src/caller.py");
        assert!(
            caller.is_some(),
            "caller file should be ranked via import propagation"
        );
        assert!(
            caller
                .unwrap()
                .reasons
                .iter()
                .any(|r| r.contains("importer-of:src/app.py"))
        );
    }
}
