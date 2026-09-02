//! AST node definitions for a parsed Papyrus script.

use serde::{Deserialize, Serialize};

pub use crate::token::IntFormat;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeName {
    pub name: String,
    pub is_array: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Script {
    pub name: String,
    pub extends: Option<String>,
    pub is_hidden: bool,
    pub is_conditional: bool,
    pub imports: Vec<String>,
    pub properties: Vec<PropertyDecl>,
    pub variables: Vec<VariableDecl>,
    pub functions: Vec<FunctionDecl>,
    pub states: Vec<StateDecl>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropertyDecl {
    pub type_name: TypeName,
    pub name: String,
    pub value: Option<Expr>,
    pub is_auto: bool,
    pub is_auto_read_only: bool,
    pub is_hidden: bool,
    pub is_conditional: bool,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariableDecl {
    pub type_name: TypeName,
    pub name: String,
    pub value: Option<Expr>,
    pub is_conditional: bool,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Param {
    pub type_name: TypeName,
    pub name: String,
    pub default: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionDecl {
    pub name: String,
    pub return_type: Option<TypeName>,
    pub params: Vec<Param>,
    pub is_global: bool,
    pub is_native: bool,
    pub is_event: bool,
    pub body: Vec<Stmt>,
    pub line: usize,
    /// The name of the `State` block this function/event is declared in, or
    /// `None` when it's declared directly on the script (the "empty state";
    /// see [`StateDecl`]). Per the language's state machine, a function
    /// declared inside a state is an override: the current state's version
    /// runs in place of the empty state's identically-named, same-signature
    /// version whenever the script is in (or falls back through) that state.
    pub state: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateDecl {
    pub name: String,
    pub is_auto: bool,
    pub functions: Vec<FunctionDecl>,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IfBranch {
    pub condition: Expr,
    pub body: Vec<Stmt>,
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Stmt {
    VarDecl(VariableDecl),
    Assign {
        target: Expr,
        op: AssignOp,
        value: Expr,
        line: usize,
    },
    Expr {
        value: Expr,
        line: usize,
    },
    Return {
        value: Option<Expr>,
        line: usize,
    },
    If {
        branches: Vec<IfBranch>,
        else_body: Vec<Stmt>,
        line: usize,
    },
    While {
        condition: Expr,
        body: Vec<Stmt>,
        line: usize,
        col: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssignOp {
    Assign,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
    ModAssign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    NotEq,
    Gt,
    Lt,
    GtEq,
    LtEq,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Literal {
    /// An integer literal, alongside the notation (decimal or `0x`/`0X`
    /// hexadecimal) it was written with in source. Preserving this lets
    /// downstream tooling (see `formid_hex_notation` in `papyrus-lints`)
    /// tell a hex-written FormID apart from a decimal one without
    /// re-scanning the original source text.
    Int {
        value: i64,
        format: IntFormat,
    },
    Float(f64),
    String(String),
    Bool(bool),
    None,
}

impl Literal {
    /// Convenience constructor for an integer literal in plain decimal
    /// notation, for callers (constant folding, tests, ...) that build a
    /// `Literal::Int` synthetically rather than parsing one from source and
    /// so have no original notation to preserve.
    pub fn int(value: i64) -> Self {
        Literal::Int {
            value,
            format: IntFormat::Decimal,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Literal(Literal),
    Identifier(String),
    Self_,
    Parent,
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        line: usize,
        col: usize,
    },
    /// A named argument in a call's argument list (`func(argB = 1)`),
    /// Papyrus's syntax for passing an argument by parameter name instead
    /// of by position. Only ever appears as an element of `Call.args`.
    NamedArg {
        name: String,
        value: Box<Expr>,
    },
    Member {
        object: Box<Expr>,
        property: String,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
    },
    Cast {
        value: Box<Expr>,
        type_name: String,
    },
    NewArray {
        type_name: TypeName,
        size: Box<Expr>,
    },
}
