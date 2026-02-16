#[derive(Debug, Clone)]
pub struct ModulePath {
    pub segments: Vec<String>,
}

impl ModulePath {
    pub fn to_file_path(&self) -> String {
        format!("{}.lean", self.segments.join("/"))
    }

}

#[derive(Debug, Clone)]
pub enum Decl {
    FunDef {
        name: String,
        params: Vec<(String, Type)>,
        return_type: Option<Type>,
        body: Expr,
    },
    FunDefMatch {
        name: String,
        params: Vec<(String, Type)>,
        return_type: Option<Type>,
        cases: Vec<(Vec<Pattern>, Expr)>,
    },
    InductiveDef {
        name: String,
        constructors: Vec<Constructor>,
    },
    StructDef {
        name: String,
        fields: Vec<(String, Type)>,
    },
    Eval(Expr),
    Import {
        path: ModulePath,
    },
    Open {
        path: ModulePath,
    },
    Namespace {
        name: String,
        decls: Vec<Decl>,
    },
}

#[derive(Debug, Clone)]
pub struct Constructor {
    pub name: String,
    pub fields: Vec<Type>,
}

use crate::error::Span;

#[derive(Debug, Clone)]
pub enum Expr {
    IntLit(u64),
    StringLit(String),
    BoolLit(bool),
    Var(String, Span),
    BinOp {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    UnaryOp {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    FunApp {
        func: Box<Expr>,
        arg: Box<Expr>,
    },
    If {
        cond: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
    },
    Let {
        name: String,
        ty: Option<Type>,
        value: Box<Expr>,
        body: Box<Expr>,
    },
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    Do(Vec<DoStatement>),
    Lambda {
        params: Vec<(String, Option<Type>)>,
        body: Box<Expr>,
    },
    Paren(Box<Expr>),
    Tuple(Vec<Expr>),
    StringInterpolation(Vec<StringInterpPart>),
}

#[derive(Debug, Clone)]
pub enum StringInterpPart {
    Literal(String),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Not,
    Neg,
}

#[derive(Debug, Clone)]
pub enum Type {
    Named(String),
    Arrow(Box<Type>, Box<Type>),
    App(Box<Type>, Box<Type>),
    Tuple(Vec<Type>),
    Unit,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    IntLit(u64),
    Var(String),
    Constructor(String, Vec<Pattern>),
    Wildcard,
    Successor(String, u64),
    Tuple(Vec<Pattern>),
}

#[derive(Debug, Clone)]
pub enum DoStatement {
    Expr(Expr),
    Let(String, Expr),
}
