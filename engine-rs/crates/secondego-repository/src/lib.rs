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
        let terms = query_terms(query);
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
            }
            if self.snapshot.test_files.contains(path)
                && terms
                    .iter()
                    .any(|term| matches!(term.as_str(), "test" | "bug" | "fix" | "regression"))
            {
                add_score(&mut scores, &mut reasons, path, 1, "test-relevance".into());
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
            if let Some(module) = path.strip_suffix(".py") {
                let dotted = module
                    .replace('/', ".")
                    .trim_end_matches(".__init__")
                    .to_owned();
                module_to_file.insert(dotted.clone(), path.clone());
                if let Some(base) = dotted.split('.').last() {
                    module_to_file
                        .entry(base.to_owned())
                        .or_insert_with(|| path.clone());
                }
            }
        }

        let scored_files: Vec<(String, i32)> = scores
            .iter()
            .filter(|&(_, &score)| score >= 3)
            .map(|(path, &score)| (path.clone(), score))
            .collect();

        for (scored_path, _) in scored_files {
            for edge in &self.imports {
                let edge_target = module_to_file.get(&edge.module).cloned();
                if edge_target.as_deref() == Some(&scored_path) && edge.source_path != scored_path {
                    add_score(
                        &mut scores,
                        &mut reasons,
                        &edge.source_path,
                        4,
                        format!("importer-of:{}", scored_path),
                    );
                } else if edge.source_path == scored_path {
                    if let Some(dep_path) = module_to_file.get(&edge.module) {
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
        let mut parser_failures = Vec::new();
        let mut tests = Vec::new();
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .map_err(|error| {
                std::io::Error::other(format!("tree-sitter language setup failed: {error}"))
            })?;

        for path in &snapshot.files {
            if !path.ends_with(".py") {
                continue;
            }
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
            collect_python_nodes(
                tree.root_node(),
                &source,
                path,
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

fn is_test_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_lowercase();
    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("py" | "js" | "jsx" | "ts" | "tsx")
    ) && (name.starts_with("test_")
        || name.ends_with("_test.py")
        || name.contains(".test.")
        || name.contains(".spec.")
        || path.components().any(|part| part.as_os_str() == "tests"))
}

fn collect_python_nodes(
    root: Node<'_>,
    source: &str,
    path: &str,
    symbols: &mut Vec<Symbol>,
    imports: &mut Vec<ImportEdge>,
    tests: &mut Vec<TestNode>,
    is_test_file: bool,
) {
    let mut cursor = root.walk();
    for node in root.children(&mut cursor) {
        let kind = node.kind();
        if matches!(kind, "function_definition" | "class_definition") {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = name_node
                    .utf8_text(source.as_bytes())
                    .unwrap_or("<invalid>")
                    .to_owned();
                let line = node.start_position().row + 1;
                symbols.push(Symbol {
                    name: name.clone(),
                    kind: if kind == "class_definition" {
                        "class"
                    } else {
                        "function"
                    }
                    .into(),
                    path: path.into(),
                    line_start: line,
                    line_end: node.end_position().row + 1,
                });
                if is_test_file && kind == "function_definition" && name.starts_with("test") {
                    tests.push(TestNode {
                        path: path.into(),
                        name,
                        line,
                    });
                }
            }
        } else if kind == "import_statement" || kind == "import_from_statement" {
            let raw = node.utf8_text(source.as_bytes()).unwrap_or_default();
            for module in parse_import_modules(raw) {
                imports.push(ImportEdge {
                    source_path: path.into(),
                    module,
                    line: node.start_position().row + 1,
                });
            }
        }
        collect_python_nodes(node, source, path, symbols, imports, tests, is_test_file);
    }
}

fn parse_import_modules(raw: &str) -> Vec<String> {
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

fn link_tests(snapshot: &RepositorySnapshot, imports: &[ImportEdge]) -> Vec<TestLink> {
    let mut module_paths = BTreeMap::new();
    for path in &snapshot.files {
        if let Some(module) = path.strip_suffix(".py") {
            let module = module
                .replace('/', ".")
                .trim_end_matches(".__init__")
                .to_owned();
            module_paths.insert(module, path.clone());
        }
    }
    let mut links = BTreeSet::new();
    for edge in imports
        .iter()
        .filter(|edge| snapshot.test_files.contains(&edge.source_path))
    {
        let target = module_paths.get(&edge.module).cloned().or_else(|| {
            module_paths
                .iter()
                .find(|(module, _)| {
                    module.ends_with(&format!(".{0}", edge.module))
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
