use crate::ast::*;
use crate::error::LustcResult;
use std::collections::{HashMap, HashSet};

/// Collect all identifier names referenced in an expression.
fn collect_names_expr(expr: &Expr, names: &mut HashSet<String>) {
    match expr {
        Expr::Var(name, _) => {
            names.insert(name.clone());
            // Also add the base part for qualified names
            if let Some(dot_pos) = name.find('.') {
                names.insert(name[..dot_pos].to_string());
            }
        }
        Expr::IntLit(_) | Expr::StringLit(_) | Expr::BoolLit(_) => {}
        Expr::BinOp { lhs, rhs, .. } => {
            collect_names_expr(lhs, names);
            collect_names_expr(rhs, names);
        }
        Expr::UnaryOp { operand, .. } => collect_names_expr(operand, names),
        Expr::FunApp { func, arg } => {
            collect_names_expr(func, names);
            collect_names_expr(arg, names);
        }
        Expr::If { cond, then_branch, else_branch } => {
            collect_names_expr(cond, names);
            collect_names_expr(then_branch, names);
            collect_names_expr(else_branch, names);
        }
        Expr::Let { value, body, .. } => {
            collect_names_expr(value, names);
            collect_names_expr(body, names);
        }
        Expr::Match { scrutinee, arms } => {
            collect_names_expr(scrutinee, names);
            for arm in arms {
                collect_names_pattern(&arm.pattern, names);
                collect_names_expr(&arm.body, names);
            }
        }
        Expr::Do(stmts) => {
            for stmt in stmts {
                match stmt {
                    DoStatement::Expr(e) => collect_names_expr(e, names),
                    DoStatement::Let(_, value) => collect_names_expr(value, names),
                }
            }
        }
        Expr::Lambda { body, .. } => collect_names_expr(body, names),
        Expr::Paren(inner) => collect_names_expr(inner, names),
        Expr::Tuple(elems) => {
            for e in elems {
                collect_names_expr(e, names);
            }
        }
        Expr::StringInterpolation(parts) => {
            for part in parts {
                if let StringInterpPart::Expr(e) = part {
                    collect_names_expr(e, names);
                }
            }
        }
    }
}

fn collect_names_pattern(pat: &Pattern, names: &mut HashSet<String>) {
    if let Pattern::Constructor(name, args) = pat {
        names.insert(name.clone());
        if let Some(dot_pos) = name.find('.') {
            names.insert(name[..dot_pos].to_string());
        }
        for arg in args {
            collect_names_pattern(arg, names);
        }
    }
}

fn collect_reachable_decls(
    decls: &[Decl],
    prefix: &str,
    decl_refs: &mut HashMap<String, HashSet<String>>,
    type_associated: &mut HashMap<String, Vec<String>>,
) {
    for decl in decls {
        match decl {
            Decl::FunDef { name, body, .. } => {
                let qualified = if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{}.{}", prefix, name)
                };
                let mut refs = HashSet::new();
                collect_names_expr(body, &mut refs);
                decl_refs.insert(qualified, refs);
            }
            Decl::FunDefMatch { name, cases, .. } => {
                let qualified = if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{}.{}", prefix, name)
                };
                let mut refs = HashSet::new();
                for (patterns, body) in cases {
                    for pat in patterns {
                        collect_names_pattern(pat, &mut refs);
                    }
                    collect_names_expr(body, &mut refs);
                }
                decl_refs.insert(qualified, refs);
            }
            Decl::InductiveDef { name, constructors } => {
                let qualified = if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{}.{}", prefix, name)
                };
                let mut associated = vec![qualified.clone()];
                for ctor in constructors {
                    associated.push(ctor.name.clone());
                    associated.push(format!("{}.{}", name, ctor.name));
                    if !prefix.is_empty() {
                        associated.push(format!("{}.{}.{}", prefix, name, ctor.name));
                    }
                }
                type_associated.insert(qualified.clone(), associated);
                decl_refs.insert(qualified, HashSet::new());
            }
            Decl::StructDef { name, fields } => {
                let qualified = if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{}.{}", prefix, name)
                };
                let mut associated = vec![qualified.clone(), format!("{}.mk", name)];
                if !prefix.is_empty() {
                    associated.push(format!("{}.{}.mk", prefix, name));
                }
                for (fname, _) in fields {
                    associated.push(format!("{}.{}", name, fname));
                    if !prefix.is_empty() {
                        associated.push(format!("{}.{}.{}", prefix, name, fname));
                    }
                }
                type_associated.insert(qualified.clone(), associated);
                decl_refs.insert(qualified, HashSet::new());
            }
            Decl::Namespace { name, decls: inner } => {
                // The namespace itself is reachable if any of its members are referenced
                let qualified = if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{}.{}", prefix, name)
                };
                // Collect inner declarations with qualified prefix
                collect_reachable_decls(inner, &qualified, decl_refs, type_associated);
                // Make the namespace name map to all its inner decls
                let mut associated = vec![qualified.clone()];
                for d in inner {
                    let inner_name = match d {
                        Decl::FunDef { name: n, .. }
                        | Decl::FunDefMatch { name: n, .. }
                        | Decl::InductiveDef { name: n, .. }
                        | Decl::StructDef { name: n, .. }
                        | Decl::Namespace { name: n, .. } => {
                            Some(format!("{}.{}", qualified, n))
                        }
                        _ => None,
                    };
                    if let Some(n) = inner_name {
                        associated.push(n);
                    }
                }
                type_associated.insert(qualified, associated);
            }
            Decl::Eval(_) | Decl::Import { .. } | Decl::Open { .. } => {}
        }
    }
}

/// Compute the set of reachable declaration names starting from entry points.
fn collect_reachable(decls: &[Decl]) -> HashSet<String> {
    // Build a map from declaration name → referenced names
    let mut decl_refs: HashMap<String, HashSet<String>> = HashMap::new();
    // Track type name → all associated names (constructors, struct, etc.)
    let mut type_associated: HashMap<String, Vec<String>> = HashMap::new();

    collect_reachable_decls(decls, "", &mut decl_refs, &mut type_associated);

    // Entry points: main, #eval
    let has_entry_point = decls.iter().any(|d| {
        matches!(d, Decl::FunDef { name, .. } | Decl::FunDefMatch { name, .. } if name == "main")
            || matches!(d, Decl::Eval(_))
    });

    // If there are no entry points, all declarations are reachable
    if !has_entry_point {
        let mut all = HashSet::new();
        fn collect_all_names(decls: &[Decl], all: &mut HashSet<String>) {
            for decl in decls {
                match decl {
                    Decl::FunDef { name, .. } | Decl::FunDefMatch { name, .. } => {
                        all.insert(name.clone());
                    }
                    Decl::InductiveDef { name, .. } | Decl::StructDef { name, .. } => {
                        all.insert(name.clone());
                    }
                    Decl::Namespace { name, decls: inner } => {
                        all.insert(name.clone());
                        collect_all_names(inner, all);
                    }
                    _ => {}
                }
            }
        }
        collect_all_names(decls, &mut all);
        return all;
    }

    let mut reachable = HashSet::new();
    let mut worklist = Vec::new();

    for decl in decls {
        match decl {
            Decl::FunDef { name, .. } | Decl::FunDefMatch { name, .. } if name == "main" => {
                reachable.insert(name.clone());
                worklist.push(name.clone());
            }
            Decl::Eval(expr) => {
                let mut refs = HashSet::new();
                collect_names_expr(expr, &mut refs);
                for r in &refs {
                    if !reachable.contains(r) {
                        reachable.insert(r.clone());
                        worklist.push(r.clone());
                    }
                }
            }
            Decl::Import { path } => {
                // Import makes the module namespace reachable
                if let Some(last) = path.segments.last() {
                    if !reachable.contains(last) {
                        reachable.insert(last.clone());
                        worklist.push(last.clone());
                    }
                }
            }
            Decl::Open { path } => {
                // Open makes the namespace reachable
                let name = path.segments.join(".");
                if !reachable.contains(&name) {
                    reachable.insert(name.clone());
                    worklist.push(name);
                }
                // Also try the single segment
                if path.segments.len() == 1 {
                    let single = &path.segments[0];
                    if !reachable.contains(single) {
                        reachable.insert(single.clone());
                        worklist.push(single.clone());
                    }
                }
            }
            _ => {}
        }
    }

    // Fixed-point: expand reachable set
    while let Some(name) = worklist.pop() {
        // Add refs from this declaration
        if let Some(refs) = decl_refs.get(&name) {
            for r in refs {
                if !reachable.contains(r) {
                    reachable.insert(r.clone());
                    worklist.push(r.clone());
                }
            }
        }
        // If this is a type name, mark all associated names as reachable
        if let Some(associated) = type_associated.get(&name) {
            for a in associated {
                if !reachable.contains(a) {
                    reachable.insert(a.clone());
                    worklist.push(a.clone());
                }
            }
        }
    }

    reachable
}

pub struct CodeGen {
    output: String,
    indent: usize,
    inductive_names: Vec<String>,
    struct_fields: HashMap<String, Vec<(String, Type)>>,
    namespace_names: HashSet<String>,
    zero_arg_fns: HashSet<String>,
    opened_namespaces: Vec<String>,
    namespace_members: HashMap<String, HashSet<String>>,
}

impl CodeGen {
    pub fn new() -> Self {
        CodeGen {
            output: String::new(),
            indent: 0,
            inductive_names: Vec::new(),
            struct_fields: HashMap::new(),
            namespace_names: HashSet::new(),
            zero_arg_fns: HashSet::new(),
            opened_namespaces: Vec::new(),
            namespace_members: HashMap::new(),
        }
    }

    pub fn generate(&mut self, decls: &[Decl]) -> LustcResult<String> {
        // Compute reachable declarations for dead code elimination
        let reachable = collect_reachable(decls);

        // First pass: collect inductive type names, struct fields, namespace names, and zero-arg fns
        self.collect_all_info(decls, "");

        // Check if there are #eval declarations that need a main function
        let has_main_def = decls.iter().any(|d| matches!(d, Decl::FunDef { name, .. } | Decl::FunDefMatch { name, .. } if name == "main"));
        let eval_exprs: Vec<&Expr> = decls
            .iter()
            .filter_map(|d| match d {
                Decl::Eval(e) => Some(e),
                _ => None,
            })
            .collect();

        for decl in decls {
            match decl {
                Decl::Eval(_) => {} // handled below
                _ => {
                    // Skip dead code: declarations not reachable from main/#eval
                    let decl_name = match decl {
                        Decl::FunDef { name, .. } | Decl::FunDefMatch { name, .. } => Some(name.as_str()),
                        Decl::InductiveDef { name, .. } | Decl::StructDef { name, .. } => Some(name.as_str()),
                        Decl::Namespace { name, .. } => Some(name.as_str()),
                        Decl::Eval(_) | Decl::Import { .. } | Decl::Open { .. } => None,
                    };
                    if let Some(name) = decl_name {
                        if !reachable.contains(name) {
                            continue;
                        }
                    }
                    self.gen_decl(decl)?;
                    self.output.push('\n');
                }
            }
        }

        // Generate main for #eval if no main exists
        if !eval_exprs.is_empty() && !has_main_def {
            self.emit_line("fn main() {");
            self.indent += 1;
            for expr in &eval_exprs {
                self.emit_indent();
                self.output.push_str("println!(\"{}\", ");
                self.gen_expr(expr)?;
                self.output.push_str(");\n");
            }
            self.indent -= 1;
            self.emit_line("}");
        }

        Ok(self.output.clone())
    }

    fn gen_decl(&mut self, decl: &Decl) -> LustcResult<()> {
        match decl {
            Decl::FunDef {
                name,
                params,
                return_type,
                body,
            } => self.gen_fun_def(name, params, return_type, body),
            Decl::FunDefMatch {
                name,
                params,
                return_type,
                cases,
            } => self.gen_fun_def_match(name, params, return_type, cases),
            Decl::InductiveDef { name, constructors } => self.gen_inductive(name, constructors),
            Decl::StructDef { name, fields } => self.gen_struct(name, fields),
            Decl::Eval(_) => Ok(()), // handled in generate()
            Decl::Import { .. } => Ok(()),
            Decl::Open { .. } => Ok(()),
            Decl::Namespace { name, decls } => self.gen_namespace(name, decls),
        }
    }

    fn gen_fun_def(
        &mut self,
        name: &str,
        params: &[(String, Type)],
        return_type: &Option<Type>,
        body: &Expr,
    ) -> LustcResult<()> {
        // Special case: main : IO Unit
        if name == "main" && is_io_unit(return_type) {
            self.emit_str("fn main()");
            self.output.push_str(" {\n");
            self.indent += 1;
            self.gen_do_body(body)?;
            self.indent -= 1;
            self.emit_line("}");
            return Ok(());
        }

        self.emit_str(&format!("fn {}", to_snake_case(name)));
        self.output.push('(');
        for (i, (pname, ptype)) in params.iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            self.output
                .push_str(&format!("{}: {}", to_snake_case(pname), self.type_to_rust(ptype)));
        }
        self.output.push(')');

        if let Some(ret) = return_type {
            if !matches!(ret, Type::Unit) {
                self.output
                    .push_str(&format!(" -> {}", self.type_to_rust(ret)));
            }
        }

        self.output.push_str(" {\n");
        self.indent += 1;
        self.emit_indent();
        self.gen_expr(body)?;
        self.output.push('\n');
        self.indent -= 1;
        self.emit_line("}");
        Ok(())
    }

    fn gen_fun_def_match(
        &mut self,
        name: &str,
        params: &[(String, Type)],
        return_type: &Option<Type>,
        cases: &[(Vec<Pattern>, Expr)],
    ) -> LustcResult<()> {
        // For pattern-matching function defs, we need to figure out the
        // match parameter. The patterns represent additional implicit args.

        // When params is empty and return_type is Arrow, decompose:
        //   def f : A → B → C  means  fn f(x: A, y: B) -> C
        // The last arrow target is the return type, everything else is a param.
        let match_param_name = self.infer_match_param_name(cases);

        let (all_params, actual_return_type) = if params.is_empty() {
            // Decompose arrow type into parameter types + return type
            let mut arrow_parts = Vec::new();
            if let Some(ref ty) = return_type {
                Self::flatten_arrow(ty, &mut arrow_parts);
            }
            if arrow_parts.len() >= 2 {
                let ret = arrow_parts.pop().unwrap();
                let param_types: Vec<(String, String)> = arrow_parts
                    .iter()
                    .enumerate()
                    .map(|(i, t)| {
                        // Use pattern variable name for the match param (last one)
                        // Use generic names for others
                        if i == arrow_parts.len() - 1 {
                            (match_param_name.clone(), self.type_to_rust(t))
                        } else {
                            (format!("x{}", i), self.type_to_rust(t))
                        }
                    })
                    .collect();
                (param_types, Some(self.type_to_rust(ret)))
            } else {
                // Single type or no type — match param uses the single type
                let param_type = arrow_parts.first().map(|t| self.type_to_rust(t));
                let p = if let Some(ty) = param_type {
                    vec![(match_param_name.clone(), ty)]
                } else {
                    vec![]
                };
                (p, None)
            }
        } else {
            // Explicit params exist, add match param from return type
            let mut all: Vec<(String, String)> = params
                .iter()
                .map(|(n, t)| (n.clone(), self.type_to_rust(t)))
                .collect();

            // If return_type is an Arrow, decompose into extra param + real return
            if let Some(ref ty) = return_type {
                let mut arrow_parts = Vec::new();
                Self::flatten_arrow(ty, &mut arrow_parts);
                if arrow_parts.len() >= 2 {
                    let ret = arrow_parts.pop().unwrap();
                    for t in &arrow_parts {
                        all.push((match_param_name.clone(), self.type_to_rust(t)));
                    }
                    (all, Some(self.type_to_rust(ret)))
                } else {
                    let ret = return_type.as_ref().map(|t| self.type_to_rust(t));
                    all.push((match_param_name.clone(), ret.clone().unwrap_or_default()));
                    (all, ret)
                }
            } else {
                (all, None)
            }
        };

        let snake_match_param = to_snake_case(&match_param_name);
        self.emit_str(&format!("fn {}", to_snake_case(name)));
        self.output.push('(');

        for (i, (pname, ptype)) in all_params.iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            self.output.push_str(&format!("{}: {}", to_snake_case(pname), ptype));
        }
        self.output.push(')');

        if let Some(ref ret) = actual_return_type {
            if ret != "()" {
                self.output.push_str(&format!(" -> {}", ret));
            }
        }

        self.output.push_str(" {\n");
        self.indent += 1;

        // Generate match expression
        self.emit_indent();
        self.output
            .push_str(&format!("match {} {{\n", snake_match_param));
        self.indent += 1;

        for (patterns, body) in cases {
            self.emit_indent();
            if let Some(pat) = patterns.first() {
                // Successor pattern: `n + k` → catch-all that rebinds n = n - k
                if let Pattern::Successor(var, k) = pat {
                    let snake_var = to_snake_case(var);
                    self.output.push_str(&format!("{} => {{\n", snake_var));
                    self.indent += 1;
                    self.emit_indent();
                    self.output
                        .push_str(&format!("let {} = {}.saturating_sub({});\n", snake_var, snake_var, k));
                    self.emit_indent();
                    self.gen_expr(body)?;
                    self.output.push('\n');
                    self.indent -= 1;
                    self.emit_indent();
                    self.output.push_str("}\n");
                    continue;
                }
                self.gen_pattern(pat)?;
            }
            self.output.push_str(" => ");
            self.gen_expr(body)?;
            self.output.push_str(",\n");
        }

        self.indent -= 1;
        self.emit_indent();
        self.output.push_str("}\n");

        self.indent -= 1;
        self.emit_line("}");
        Ok(())
    }

    fn flatten_arrow<'a>(ty: &'a Type, parts: &mut Vec<&'a Type>) {
        match ty {
            Type::Arrow(lhs, rhs) => {
                parts.push(lhs.as_ref());
                Self::flatten_arrow(rhs.as_ref(), parts);
            }
            _ => {
                parts.push(ty);
            }
        }
    }

    fn infer_match_param_name(&self, cases: &[(Vec<Pattern>, Expr)]) -> String {
        for (patterns, _) in cases {
            if let Some(pat) = patterns.first() {
                match pat {
                    Pattern::Var(name) => return name.clone(),
                    Pattern::Successor(name, _) => return name.clone(),
                    _ => {}
                }
            }
        }
        "x".to_string()
    }

    fn gen_inductive(&mut self, name: &str, constructors: &[Constructor]) -> LustcResult<()> {
        self.emit_line("#[derive(Debug, Clone, PartialEq)]");
        self.emit_line(&format!("enum {} {{", name));
        self.indent += 1;
        for ctor in constructors {
            self.emit_indent();
            let pascal_name = to_pascal_case(&ctor.name);
            if ctor.fields.is_empty() {
                self.output.push_str(&format!("{},\n", pascal_name));
            } else {
                self.output.push_str(&format!("{}(", pascal_name));
                for (i, field) in ctor.fields.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.output.push_str(&self.type_to_rust(field));
                }
                self.output.push_str("),\n");
            }
        }
        self.indent -= 1;
        self.emit_line("}");
        Ok(())
    }

    fn gen_struct(&mut self, name: &str, fields: &[(String, Type)]) -> LustcResult<()> {
        self.emit_line("#[derive(Debug, Clone, PartialEq)]");
        self.emit_line(&format!("struct {} {{", name));
        self.indent += 1;
        for (fname, ftype) in fields {
            self.emit_indent();
            self.output
                .push_str(&format!("{}: {},\n", fname, self.type_to_rust(ftype)));
        }
        self.indent -= 1;
        self.emit_line("}");
        Ok(())
    }

    fn collect_all_info(&mut self, decls: &[Decl], prefix: &str) {
        for decl in decls {
            match decl {
                Decl::FunDef { name, params, .. } => {
                    if params.is_empty() && name != "main" {
                        self.zero_arg_fns.insert(name.clone());
                        if !prefix.is_empty() {
                            self.zero_arg_fns.insert(format!("{}.{}", prefix, name));
                        }
                    }
                }
                Decl::FunDefMatch { name, params, return_type, .. } => {
                    // FunDefMatch with no explicit params but an Arrow return type
                    // actually takes params from the arrow decomposition — not zero-arg
                    if params.is_empty() && name != "main" {
                        let has_arrow_params = matches!(return_type, Some(Type::Arrow(_, _)));
                        if !has_arrow_params {
                            self.zero_arg_fns.insert(name.clone());
                            if !prefix.is_empty() {
                                self.zero_arg_fns.insert(format!("{}.{}", prefix, name));
                            }
                        }
                    }
                }
                Decl::InductiveDef { name, .. } => {
                    self.inductive_names.push(name.clone());
                }
                Decl::StructDef { name, fields } => {
                    self.struct_fields.insert(name.clone(), fields.clone());
                }
                Decl::Namespace { name, decls: inner } => {
                    self.namespace_names.insert(name.clone());
                    // Collect namespace members
                    let mut members = HashSet::new();
                    for d in inner {
                        match d {
                            Decl::FunDef { name: n, .. }
                            | Decl::FunDefMatch { name: n, .. }
                            | Decl::InductiveDef { name: n, .. }
                            | Decl::StructDef { name: n, .. } => {
                                members.insert(n.clone());
                            }
                            _ => {}
                        }
                    }
                    self.namespace_members.insert(name.clone(), members);

                    let new_prefix = if prefix.is_empty() {
                        name.clone()
                    } else {
                        format!("{}.{}", prefix, name)
                    };
                    self.collect_all_info(inner, &new_prefix);
                }
                Decl::Import { .. } => {
                    // Import implicitly opens the last segment
                    // handled below when processing decls
                }
                Decl::Open { .. } | Decl::Eval(_) => {}
            }
        }

        // Second pass: process import/open for opened namespaces
        for decl in decls {
            match decl {
                Decl::Import { path } => {
                    // Import opens the last segment namespace
                    let last = path.segments.last().cloned().unwrap_or_default();
                    if !last.is_empty() {
                        self.opened_namespaces.push(last);
                    }
                }
                Decl::Open { path } => {
                    let name = path.segments.join(".");
                    // For single-segment paths, use directly
                    if path.segments.len() == 1 {
                        self.opened_namespaces.push(path.segments[0].clone());
                    } else {
                        self.opened_namespaces.push(name);
                    }
                }
                _ => {}
            }
        }
    }

    fn gen_namespace(&mut self, name: &str, decls: &[Decl]) -> LustcResult<()> {
        let rust_name = to_snake_case(name);
        self.emit_line(&format!("mod {} {{", rust_name));
        self.indent += 1;
        for decl in decls {
            self.gen_decl_pub(decl)?;
            self.output.push('\n');
        }
        self.indent -= 1;
        self.emit_line("}");
        Ok(())
    }

    fn gen_decl_pub(&mut self, decl: &Decl) -> LustcResult<()> {
        match decl {
            Decl::FunDef {
                name,
                params,
                return_type,
                body,
            } => {
                // Special case: main : IO Unit
                if name == "main" && is_io_unit(return_type) {
                    self.emit_str("pub fn main()");
                    self.output.push_str(" {\n");
                    self.indent += 1;
                    self.gen_do_body(body)?;
                    self.indent -= 1;
                    self.emit_line("}");
                    return Ok(());
                }

                self.emit_str(&format!("pub fn {}", to_snake_case(name)));
                self.output.push('(');
                for (i, (pname, ptype)) in params.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.output
                        .push_str(&format!("{}: {}", to_snake_case(pname), self.type_to_rust(ptype)));
                }
                self.output.push(')');

                if let Some(ret) = return_type {
                    if !matches!(ret, Type::Unit) {
                        self.output
                            .push_str(&format!(" -> {}", self.type_to_rust(ret)));
                    }
                }

                self.output.push_str(" {\n");
                self.indent += 1;
                self.emit_indent();
                self.gen_expr(body)?;
                self.output.push('\n');
                self.indent -= 1;
                self.emit_line("}");
                Ok(())
            }
            Decl::FunDefMatch {
                name,
                params,
                return_type,
                cases,
            } => {
                // Reuse the existing logic but prefix with pub
                // For simplicity, generate as pub fn
                let match_param_name = self.infer_match_param_name(cases);

                let (all_params, actual_return_type) = if params.is_empty() {
                    let mut arrow_parts = Vec::new();
                    if let Some(ref ty) = return_type {
                        Self::flatten_arrow(ty, &mut arrow_parts);
                    }
                    if arrow_parts.len() >= 2 {
                        let ret = arrow_parts.pop().unwrap();
                        let param_types: Vec<(String, String)> = arrow_parts
                            .iter()
                            .enumerate()
                            .map(|(i, t)| {
                                if i == arrow_parts.len() - 1 {
                                    (match_param_name.clone(), self.type_to_rust(t))
                                } else {
                                    (format!("x{}", i), self.type_to_rust(t))
                                }
                            })
                            .collect();
                        (param_types, Some(self.type_to_rust(ret)))
                    } else {
                        let param_type = arrow_parts.first().map(|t| self.type_to_rust(t));
                        let p = if let Some(ty) = param_type {
                            vec![(match_param_name.clone(), ty)]
                        } else {
                            vec![]
                        };
                        (p, None)
                    }
                } else {
                    let mut all: Vec<(String, String)> = params
                        .iter()
                        .map(|(n, t)| (n.clone(), self.type_to_rust(t)))
                        .collect();
                    if let Some(ref ty) = return_type {
                        let mut arrow_parts = Vec::new();
                        Self::flatten_arrow(ty, &mut arrow_parts);
                        if arrow_parts.len() >= 2 {
                            let ret = arrow_parts.pop().unwrap();
                            for t in &arrow_parts {
                                all.push((match_param_name.clone(), self.type_to_rust(t)));
                            }
                            (all, Some(self.type_to_rust(ret)))
                        } else {
                            let ret = return_type.as_ref().map(|t| self.type_to_rust(t));
                            all.push((match_param_name.clone(), ret.clone().unwrap_or_default()));
                            (all, ret)
                        }
                    } else {
                        (all, None)
                    }
                };

                let snake_match_param = to_snake_case(&match_param_name);
                self.emit_str(&format!("pub fn {}", to_snake_case(name)));
                self.output.push('(');
                for (i, (pname, ptype)) in all_params.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.output.push_str(&format!("{}: {}", to_snake_case(pname), ptype));
                }
                self.output.push(')');
                if let Some(ref ret) = actual_return_type {
                    if ret != "()" {
                        self.output.push_str(&format!(" -> {}", ret));
                    }
                }
                self.output.push_str(" {\n");
                self.indent += 1;
                self.emit_indent();
                self.output
                    .push_str(&format!("match {} {{\n", snake_match_param));
                self.indent += 1;
                for (patterns, body) in cases {
                    self.emit_indent();
                    if let Some(pat) = patterns.first() {
                        if let Pattern::Successor(var, k) = pat {
                            let snake_var = to_snake_case(var);
                            self.output.push_str(&format!("{} => {{\n", snake_var));
                            self.indent += 1;
                            self.emit_indent();
                            self.output
                                .push_str(&format!("let {} = {}.saturating_sub({});\n", snake_var, snake_var, k));
                            self.emit_indent();
                            self.gen_expr(body)?;
                            self.output.push('\n');
                            self.indent -= 1;
                            self.emit_indent();
                            self.output.push_str("}\n");
                            continue;
                        }
                        self.gen_pattern(pat)?;
                    }
                    self.output.push_str(" => ");
                    self.gen_expr(body)?;
                    self.output.push_str(",\n");
                }
                self.indent -= 1;
                self.emit_indent();
                self.output.push_str("}\n");
                self.indent -= 1;
                self.emit_line("}");
                Ok(())
            }
            Decl::InductiveDef { name, constructors } => {
                self.emit_line("#[derive(Debug, Clone, PartialEq)]");
                self.emit_line(&format!("pub enum {} {{", name));
                self.indent += 1;
                for ctor in constructors {
                    self.emit_indent();
                    let pascal_name = to_pascal_case(&ctor.name);
                    if ctor.fields.is_empty() {
                        self.output.push_str(&format!("{},\n", pascal_name));
                    } else {
                        self.output.push_str(&format!("{}(", pascal_name));
                        for (i, field) in ctor.fields.iter().enumerate() {
                            if i > 0 {
                                self.output.push_str(", ");
                            }
                            self.output.push_str(&self.type_to_rust(field));
                        }
                        self.output.push_str("),\n");
                    }
                }
                self.indent -= 1;
                self.emit_line("}");
                Ok(())
            }
            Decl::StructDef { name, fields } => {
                self.emit_line("#[derive(Debug, Clone, PartialEq)]");
                self.emit_line(&format!("pub struct {} {{", name));
                self.indent += 1;
                for (fname, ftype) in fields {
                    self.emit_indent();
                    self.output
                        .push_str(&format!("pub {}: {},\n", fname, self.type_to_rust(ftype)));
                }
                self.indent -= 1;
                self.emit_line("}");
                Ok(())
            }
            Decl::Eval(_) => Ok(()),
            Decl::Import { .. } => Ok(()),
            Decl::Open { .. } => Ok(()),
            Decl::Namespace { name, decls } => self.gen_namespace(name, decls),
        }
    }

    fn gen_expr(&mut self, expr: &Expr) -> LustcResult<()> {
        match expr {
            Expr::IntLit(n) => self.output.push_str(&n.to_string()),
            Expr::StringLit(s) => {
                self.output.push_str("String::from(\"");
                self.output
                    .push_str(&s.replace('\\', "\\\\").replace('"', "\\\""));
                self.output.push_str("\")");
            }
            Expr::BoolLit(b) => self.output.push_str(if *b { "true" } else { "false" }),
            Expr::Var(name, _span) => {
                self.output.push_str(&self.translate_var(name));
            }
            Expr::BinOp { op, lhs, rhs } => {
                // Special case: Nat subtraction uses saturating_sub
                if *op == BinOp::Sub {
                    self.gen_expr(lhs)?;
                    self.output.push_str(".saturating_sub(");
                    self.gen_expr(rhs)?;
                    self.output.push(')');
                } else {
                    self.gen_expr(lhs)?;
                    self.output.push_str(&format!(" {} ", binop_to_rust(op)));
                    self.gen_expr(rhs)?;
                }
            }
            Expr::UnaryOp { op, operand } => {
                match op {
                    UnaryOp::Not => self.output.push('!'),
                    UnaryOp::Neg => self.output.push('-'),
                }
                self.gen_expr(operand)?;
            }
            Expr::FunApp { func, arg } => {
                self.gen_fun_app(func, arg)?;
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.output.push_str("if ");
                self.gen_expr(cond)?;
                self.output.push_str(" { ");
                self.gen_expr(then_branch)?;
                self.output.push_str(" } else { ");
                self.gen_expr(else_branch)?;
                self.output.push_str(" }");
            }
            Expr::Let {
                name, value, body, ..
            } => {
                self.output.push_str(&format!("{{ let {} = ", to_snake_case(name)));
                self.gen_expr(value)?;
                self.output.push_str("; ");
                self.gen_expr(body)?;
                self.output.push_str(" }");
            }
            Expr::Match { scrutinee, arms } => {
                self.output.push_str("match ");
                self.gen_expr(scrutinee)?;
                self.output.push_str(" {\n");
                self.indent += 1;
                for arm in arms {
                    self.emit_indent();
                    self.gen_pattern(&arm.pattern)?;
                    self.output.push_str(" => ");
                    self.gen_expr(&arm.body)?;
                    self.output.push_str(",\n");
                }
                self.indent -= 1;
                self.emit_indent();
                self.output.push('}');
            }
            Expr::Do(stmts) => {
                self.gen_do_stmts(stmts)?;
            }
            Expr::Lambda { params, body } => {
                self.output.push('|');
                for (i, (name, _)) in params.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.output.push_str(&to_snake_case(name));
                }
                self.output.push_str("| ");
                self.gen_expr(body)?;
            }
            Expr::Paren(inner) => {
                if self.is_atom_expr(inner) {
                    self.gen_expr(inner)?;
                } else {
                    self.output.push('(');
                    self.gen_expr(inner)?;
                    self.output.push(')');
                }
            }
            Expr::Tuple(elems) => {
                self.output.push('(');
                for (i, e) in elems.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.gen_expr(e)?;
                }
                self.output.push(')');
            }
            Expr::StringInterpolation(parts) => {
                self.output.push_str("format!(\"");
                let mut exprs = Vec::new();
                for part in parts {
                    match part {
                        StringInterpPart::Literal(s) => {
                            // Escape braces for format! string
                            self.output.push_str(
                                &s.replace('{', "{{")
                                    .replace('}', "}}")
                                    .replace('\\', "\\\\")
                                    .replace('"', "\\\""),
                            );
                        }
                        StringInterpPart::Expr(e) => {
                            self.output.push_str("{}");
                            exprs.push(e);
                        }
                    }
                }
                self.output.push('"');
                for e in exprs {
                    self.output.push_str(", ");
                    self.gen_expr(e)?;
                }
                self.output.push(')');
            }
        }
        Ok(())
    }

    fn gen_fun_app(&mut self, func: &Expr, arg: &Expr) -> LustcResult<()> {
        // Flatten curried application: f x y → f(x, y)
        let mut args = vec![arg];
        let mut base = func;
        while let Expr::FunApp {
            func: inner_func,
            arg: inner_arg,
        } = base
        {
            args.push(inner_arg);
            base = inner_func;
        }
        args.reverse();

        // Special cases
        if let Expr::Var(name, _span) = base {
            // IO.println → println!("{}", ...)
            if name == "IO.println" {
                self.output.push_str("println!(\"{}\", ");
                self.gen_expr(args[0])?;
                self.output.push(')');
                return Ok(());
            }

            // toString → .to_string() (skip if arg is already a String)
            if name == "toString" {
                if self.is_string_expr(args[0]) {
                    self.gen_expr(args[0])?;
                } else {
                    self.gen_expr(args[0])?;
                    self.output.push_str(".to_string()");
                }
                return Ok(());
            }

            // List builtins
            if name == "List.nil" {
                self.output.push_str("vec![]");
                return Ok(());
            }
            if name == "List.cons" && args.len() >= 2 {
                self.output.push_str("{ let mut __v = ");
                self.gen_expr(args[1])?;
                self.output.push_str(".clone(); __v.insert(0, ");
                self.gen_expr(args[0])?;
                self.output.push_str("); __v }");
                return Ok(());
            }

            // Option builtins
            if name == "Option.none" {
                self.output.push_str("None");
                return Ok(());
            }
            if name == "Option.some" {
                self.output.push_str("Some(");
                self.gen_expr(args[0])?;
                self.output.push(')');
                return Ok(());
            }

            // Struct constructor: Name.mk → Name { f1: a1, f2: a2, ... }
            if let Some(dot_pos) = name.find('.') {
                let type_name = &name[..dot_pos];
                let method = &name[dot_pos + 1..];
                if method == "mk" {
                    if let Some(fields) = self.struct_fields.get(type_name).cloned() {
                        self.output.push_str(&format!("{} {{ ", type_name));
                        for (i, ((fname, _), arg_expr)) in
                            fields.iter().zip(args.iter()).enumerate()
                        {
                            if i > 0 {
                                self.output.push_str(", ");
                            }
                            self.output.push_str(&format!("{}: ", fname));
                            self.gen_expr(arg_expr)?;
                        }
                        self.output.push_str(" }");
                        return Ok(());
                    }
                }
                // Struct field accessor: Name.field arg → arg.field
                if self.struct_fields.contains_key(type_name) {
                    if let Some(fields) = self.struct_fields.get(type_name) {
                        if fields.iter().any(|(f, _)| f == method) {
                            self.gen_expr(args[0])?;
                            self.output.push_str(&format!(".{}", method));
                            return Ok(());
                        }
                    }
                }
            }

            // Constructor: qualified name like Color.red → Color::Red
            let translated = self.translate_var(name);
            if translated.contains("::") {
                self.output.push_str(&translated);
                if !args.is_empty() && !matches!(args[0], Expr::Var(ref v, _) if v == "()") {
                    self.output.push('(');
                    for (i, a) in args.iter().enumerate() {
                        if i > 0 {
                            self.output.push_str(", ");
                        }
                        self.gen_expr(a)?;
                    }
                    self.output.push(')');
                }
                return Ok(());
            }

            // Regular function call (snake_case already applied by translate_var)
            self.output.push_str(&translated);
            self.output.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    self.output.push_str(", ");
                }
                self.gen_expr(a)?;
            }
            self.output.push(')');
            return Ok(());
        }

        // General case
        self.gen_expr(base)?;
        self.output.push('(');
        for (i, a) in args.iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            self.gen_expr(a)?;
        }
        self.output.push(')');
        Ok(())
    }

    fn gen_do_body(&mut self, body: &Expr) -> LustcResult<()> {
        match body {
            Expr::Do(stmts) => self.gen_do_stmts(stmts),
            other => {
                self.emit_indent();
                self.gen_expr(other)?;
                self.output.push_str(";\n");
                Ok(())
            }
        }
    }

    fn gen_do_stmts(&mut self, stmts: &[DoStatement]) -> LustcResult<()> {
        for stmt in stmts {
            match stmt {
                DoStatement::Expr(expr) => {
                    self.emit_indent();
                    self.gen_expr(expr)?;
                    self.output.push_str(";\n");
                }
                DoStatement::Let(name, value) => {
                    self.emit_indent();
                    self.output.push_str(&format!("let {} = ", to_snake_case(name)));
                    self.gen_expr(value)?;
                    self.output.push_str(";\n");
                }
            }
        }
        Ok(())
    }

    fn gen_pattern(&mut self, pat: &Pattern) -> LustcResult<()> {
        match pat {
            Pattern::IntLit(n) => self.output.push_str(&n.to_string()),
            Pattern::Var(name) => self.output.push_str(&to_snake_case(name)),
            Pattern::Constructor(name, args) => {
                let translated = self.translate_constructor(name);
                // Option pattern mapping
                if translated == "None" || name == "Option.none" {
                    self.output.push_str("None");
                    return Ok(());
                }
                if translated.starts_with("Some") || name == "Option.some" {
                    self.output.push_str("Some");
                    if !args.is_empty() {
                        self.output.push('(');
                        for (i, arg) in args.iter().enumerate() {
                            if i > 0 {
                                self.output.push_str(", ");
                            }
                            self.gen_pattern(arg)?;
                        }
                        self.output.push(')');
                    }
                    return Ok(());
                }
                self.output.push_str(&translated);
                if !args.is_empty() {
                    self.output.push('(');
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            self.output.push_str(", ");
                        }
                        self.gen_pattern(arg)?;
                    }
                    self.output.push(')');
                }
            }
            Pattern::Wildcard => self.output.push('_'),
            Pattern::Successor(name, _k) => {
                // In match expressions, successor patterns become catch-all bindings
                self.output.push_str(&to_snake_case(name));
            }
            Pattern::Tuple(pats) => {
                self.output.push('(');
                for (i, p) in pats.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.gen_pattern(p)?;
                }
                self.output.push(')');
            }
        }
        Ok(())
    }

    // --- Helpers ---

    /// Check if an expression is known to produce a String type
    fn is_string_expr(&self, expr: &Expr) -> bool {
        matches!(
            expr,
            Expr::StringLit(_) | Expr::StringInterpolation(_)
        )
    }

    /// Check if an expression is a simple atom that doesn't need parentheses
    fn is_atom_expr(&self, expr: &Expr) -> bool {
        matches!(
            expr,
            Expr::IntLit(_)
                | Expr::StringLit(_)
                | Expr::BoolLit(_)
                | Expr::Var(_, _)
                | Expr::Tuple(_)
        )
    }

    fn type_to_rust(&self, ty: &Type) -> String {
        match ty {
            Type::Named(name) => match name.as_str() {
                "Nat" => "u64".to_string(),
                "Int" => "i64".to_string(),
                "Bool" => "bool".to_string(),
                "String" => "String".to_string(),
                other => other.to_string(),
            },
            Type::Arrow(from, to) => {
                format!(
                    "fn({}) -> {}",
                    self.type_to_rust(from),
                    self.type_to_rust(to)
                )
            }
            Type::App(base, arg) => {
                // IO Unit → () for return types, etc.
                if let Type::Named(name) = base.as_ref() {
                    if name == "IO" {
                        return "()".to_string();
                    }
                    if name == "List" {
                        return format!("Vec<{}>", self.type_to_rust(arg));
                    }
                    if name == "Option" {
                        return format!("Option<{}>", self.type_to_rust(arg));
                    }
                }
                self.type_to_rust(base)
            }
            Type::Tuple(types) => {
                let inner: Vec<String> = types.iter().map(|t| self.type_to_rust(t)).collect();
                format!("({})", inner.join(", "))
            }
            Type::Unit => "()".to_string(),
        }
    }

    fn translate_var(&self, name: &str) -> String {
        // Handle tuple field access: p.1 → p.0 (1-indexed → 0-indexed)
        if let Some(dot_pos) = name.rfind('.') {
            let obj = &name[..dot_pos];
            let field = &name[dot_pos + 1..];
            if let Ok(idx) = field.parse::<usize>() {
                let rust_idx = idx.saturating_sub(1);
                return format!("{}.{}", to_snake_case(obj), rust_idx);
            }
            // Check for inductive constructor
            let type_name = &name[..dot_pos];
            let ctor_name = &name[dot_pos + 1..];
            if self.inductive_names.contains(&type_name.to_string()) {
                return format!("{}::{}", type_name, to_pascal_case(ctor_name));
            }
            // Check for namespace-qualified name
            if self.namespace_names.contains(type_name) {
                let translated = format!("{}::{}", to_snake_case(type_name), to_snake_case(ctor_name));
                if self.zero_arg_fns.contains(name) {
                    return format!("{}()", translated);
                }
                return translated;
            }
        }

        // List/Option builtins that appear as bare Var (no args)
        match name {
            "List.nil" => return "vec![]".to_string(),
            "Option.none" => return "None".to_string(),
            _ => {}
        }

        // Check if an unqualified name matches an opened namespace member
        if !name.contains('.') {
            for ns in &self.opened_namespaces {
                if let Some(members) = self.namespace_members.get(ns) {
                    if members.contains(name) {
                        let translated = format!("{}::{}", to_snake_case(ns), to_snake_case(name));
                        let qualified = format!("{}.{}", ns, name);
                        if self.zero_arg_fns.contains(&qualified) || self.zero_arg_fns.contains(name) {
                            return format!("{}()", translated);
                        }
                        return translated;
                    }
                }
            }
        }

        let translated = to_snake_case(name);
        // Handle zero-arg function calls for top-level defs
        if self.zero_arg_fns.contains(name) {
            return format!("{}()", translated);
        }
        translated
    }

    fn translate_constructor(&self, name: &str) -> String {
        // Option constructor patterns
        if name == "Option.none" {
            return "None".to_string();
        }
        if name == "Option.some" {
            return "Some".to_string();
        }
        // Handle qualified constructor names like Color.red → Color::Red
        if let Some(dot_pos) = name.find('.') {
            let type_name = &name[..dot_pos];
            let ctor_name = &name[dot_pos + 1..];
            return format!("{}::{}", type_name, to_pascal_case(ctor_name));
        }
        // Unqualified constructor
        to_pascal_case(name)
    }

    fn emit_str(&mut self, s: &str) {
        self.emit_indent();
        self.output.push_str(s);
    }

    fn emit_line(&mut self, s: &str) {
        self.emit_indent();
        self.output.push_str(s);
        self.output.push('\n');
    }

    fn emit_indent(&mut self) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
    }
}

fn is_io_unit(ty: &Option<Type>) -> bool {
    match ty {
        Some(Type::App(base, arg)) => {
            let is_io = matches!(base.as_ref(), Type::Named(n) if n == "IO");
            let is_unit = matches!(arg.as_ref(), Type::Unit)
                || matches!(arg.as_ref(), Type::Named(n) if n == "Unit");
            is_io && is_unit
        }
        _ => false,
    }
}

fn binop_to_rust(op: &BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-", // Note: usually handled by saturating_sub
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Mod => "%",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
    }
}

fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(ch.to_ascii_lowercase());
        } else {
            result.push(ch);
        }
    }
    result
}

fn to_pascal_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;
    for ch in s.chars() {
        if ch == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(ch.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn compile(src: &str) -> String {
        let mut lexer = Lexer::new(src);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let decls = parser.parse_program().unwrap();
        let mut codegen = CodeGen::new();
        codegen.generate(&decls).unwrap()
    }

    #[test]
    fn test_simple_function() {
        let result = compile("def add (x : Nat) (y : Nat) : Nat := x + y");
        assert!(result.contains("fn add(x: u64, y: u64) -> u64"));
    }

    #[test]
    fn test_eval_generates_main() {
        let result = compile("#eval 1 + 2");
        assert!(result.contains("fn main()"));
        assert!(result.contains("println!"));
    }

    #[test]
    fn test_inductive_type() {
        let result = compile("inductive Color where\n  | red\n  | green\n  | blue");
        assert!(result.contains("enum Color"));
        assert!(result.contains("Red"));
        assert!(result.contains("Green"));
        assert!(result.contains("Blue"));
        assert!(result.contains("#[derive(Debug, Clone, PartialEq)]"));
    }

    #[test]
    fn test_pascal_case() {
        assert_eq!(to_pascal_case("red"), "Red");
        assert_eq!(to_pascal_case("light_blue"), "LightBlue");
        assert_eq!(to_pascal_case("hello"), "Hello");
    }

    #[test]
    fn test_nat_subtraction_saturating() {
        let result = compile("def pred (n : Nat) : Nat := n - 1");
        assert!(result.contains("saturating_sub"));
    }

    #[test]
    fn test_io_println() {
        let result = compile("def main : IO Unit := do\n  IO.println \"Hello!\"");
        assert!(result.contains("fn main()"));
        assert!(result.contains("println!"));
        assert!(result.contains("Hello!"));
    }

    #[test]
    fn test_type_conversion() {
        let cg = CodeGen::new();
        assert_eq!(cg.type_to_rust(&Type::Named("Nat".to_string())), "u64");
        assert_eq!(cg.type_to_rust(&Type::Named("Int".to_string())), "i64");
        assert_eq!(cg.type_to_rust(&Type::Named("Bool".to_string())), "bool");
        assert_eq!(
            cg.type_to_rust(&Type::Named("String".to_string())),
            "String"
        );
        assert_eq!(cg.type_to_rust(&Type::Unit), "()");
    }

    #[test]
    fn test_struct_generation() {
        let result = compile("structure Point where\n  x : Nat\n  y : Nat");
        assert!(result.contains("struct Point"));
        assert!(result.contains("x: u64"));
        assert!(result.contains("y: u64"));
        assert!(result.contains("#[derive(Debug, Clone, PartialEq)]"));
    }

    #[test]
    fn test_tuple_type_conversion() {
        let cg = CodeGen::new();
        let tuple_type = Type::Tuple(vec![
            Type::Named("Nat".to_string()),
            Type::Named("Bool".to_string()),
        ]);
        assert_eq!(cg.type_to_rust(&tuple_type), "(u64, bool)");
    }

    #[test]
    fn test_list_type_conversion() {
        let cg = CodeGen::new();
        let list_type = Type::App(
            Box::new(Type::Named("List".to_string())),
            Box::new(Type::Named("Nat".to_string())),
        );
        assert_eq!(cg.type_to_rust(&list_type), "Vec<u64>");
    }

    #[test]
    fn test_option_type_conversion() {
        let cg = CodeGen::new();
        let opt_type = Type::App(
            Box::new(Type::Named("Option".to_string())),
            Box::new(Type::Named("Nat".to_string())),
        );
        assert_eq!(cg.type_to_rust(&opt_type), "Option<u64>");
    }

    #[test]
    fn test_string_interpolation_codegen() {
        let result = compile(r#"def main : IO Unit := do
  let name := "world"
  IO.println s!"Hello, {name}!""#);
        assert!(result.contains("format!("));
        assert!(result.contains("Hello, {}!"));
    }

    #[test]
    fn test_snake_case() {
        assert_eq!(to_snake_case("circleArea"), "circle_area");
        assert_eq!(to_snake_case("colorName"), "color_name");
        assert_eq!(to_snake_case("factorial"), "factorial");
        assert_eq!(to_snake_case("main"), "main");
        assert_eq!(to_snake_case("x"), "x");
        assert_eq!(to_snake_case("showOpt"), "show_opt");
    }

    #[test]
    fn test_snake_case_in_output() {
        let result = compile("def circleArea (r : Nat) : Nat := r * r * 3");
        assert!(result.contains("fn circle_area(r: u64) -> u64"));
    }

    #[test]
    fn test_dead_code_elimination() {
        let result = compile(
            "def unused (x : Nat) : Nat := x + 1\ndef helper (x : Nat) : Nat := x * 2\ndef main : IO Unit := do\n  IO.println (toString (helper 5))"
        );
        assert!(!result.contains("fn unused"));
        assert!(result.contains("helper"));
    }
}
