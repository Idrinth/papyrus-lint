//! AST node definitions for a parsed Papyrus script.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TypeName {
    pub name: String,
    pub is_array: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VariableDecl {
    pub type_name: TypeName,
    pub name: String,
    pub value: Option<Expr>,
    pub is_conditional: bool,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Param {
    pub type_name: TypeName,
    pub name: String,
    pub default: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FunctionDecl {
    pub name: String,
    pub return_type: Option<TypeName>,
    pub params: Vec<Param>,
    pub is_global: bool,
    pub is_native: bool,
    pub is_event: bool,
    pub body: Vec<Stmt>,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StateDecl {
    pub name: String,
    pub is_auto: bool,
    pub functions: Vec<FunctionDecl>,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IfBranch {
    pub condition: Expr,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Stmt {
    VarDecl(VariableDecl),
    Assign {
        target: Expr,
        op: AssignOp,
        value: Expr,
        line: usize,
    },
    Expr {
        expr: Expr,
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
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum AssignOp {
    Assign,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
    ModAssign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Literal {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
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
