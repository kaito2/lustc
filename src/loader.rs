use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::ast::{Decl, ModulePath};
use crate::error::{CompilerError, LustcResult};
use crate::lexer::Lexer;
use crate::parser::Parser;

pub struct ModuleLoader {
    base_dir: PathBuf,
    loaded: HashMap<PathBuf, Vec<Decl>>,
    loading: HashSet<PathBuf>,
    pub file_sources: Vec<(String, String)>,
}

impl ModuleLoader {
    pub fn new(entry_file: &Path) -> Self {
        let base_dir = entry_file
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        ModuleLoader {
            base_dir,
            loaded: HashMap::new(),
            loading: HashSet::new(),
            file_sources: Vec::new(),
        }
    }

    pub fn load_program(&mut self, entry_file: &Path) -> LustcResult<Vec<Decl>> {
        let source = std::fs::read_to_string(entry_file)?;
        let filename = entry_file.display().to_string();
        self.file_sources.push((filename, source.clone()));

        let mut lexer = Lexer::new(&source);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens);
        let decls = parser.parse_program()?;

        // Process imports: load imported modules and wrap in namespaces
        let mut result = Vec::new();
        for decl in &decls {
            if let Decl::Import { path } = decl {
                let imported = self.load_module(path)?;
                result.extend(imported);
            }
        }

        // Add root declarations (non-import)
        for decl in decls {
            match &decl {
                Decl::Import { .. } => {} // already processed
                _ => result.push(decl),
            }
        }

        Ok(result)
    }

    fn load_module(&mut self, path: &ModulePath) -> LustcResult<Vec<Decl>> {
        let file_path = self.base_dir.join(path.to_file_path());
        let canonical = file_path
            .canonicalize()
            .unwrap_or_else(|_| file_path.clone());

        // Check for circular imports
        if self.loading.contains(&canonical) {
            return Err(CompilerError::ModuleError {
                msg: format!(
                    "circular import detected: {}",
                    path.segments.join(".")
                ),
            });
        }

        // Check if already loaded
        if let Some(decls) = self.loaded.get(&canonical) {
            return Ok(decls.clone());
        }

        // Mark as loading
        self.loading.insert(canonical.clone());

        // Read and parse the module file
        let source = std::fs::read_to_string(&file_path).map_err(|_| {
            CompilerError::ModuleError {
                msg: format!(
                    "module `{}` not found (looked for {})",
                    path.segments.join("."),
                    file_path.display()
                ),
            }
        })?;
        let filename = file_path.display().to_string();
        self.file_sources.push((filename, source.clone()));

        let mut lexer = Lexer::new(&source);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens);
        let module_decls = parser.parse_program()?;

        // Recursively process imports within the module
        let mut processed_decls = Vec::new();
        for decl in &module_decls {
            if let Decl::Import { path: sub_path } = decl {
                let imported = self.load_module(sub_path)?;
                processed_decls.extend(imported);
            }
        }
        for decl in module_decls {
            match &decl {
                Decl::Import { .. } => {}
                _ => processed_decls.push(decl),
            }
        }

        // Unmark loading
        self.loading.remove(&canonical);

        // Wrap in namespace based on the last segment (module name)
        let last_segment = path.segments.last().cloned().unwrap_or_default();
        let wrapped = if last_segment.is_empty() {
            processed_decls
        } else {
            vec![Decl::Namespace {
                name: last_segment,
                decls: processed_decls,
            }]
        };

        // Cache
        self.loaded.insert(canonical, wrapped.clone());

        Ok(wrapped)
    }
}
