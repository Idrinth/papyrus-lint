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
    /// Whether the `ScriptName` line itself carries the `Native` flag,
    /// meaning the whole script is implemented natively by the engine
    /// with no Papyrus body of its own (e.g. Fallout 4's own `Actor.psc`,
    /// `ObjectReference.psc`, `Form.psc`). Distinct from
    /// [`FunctionDecl::is_native`], which flags one native function
    /// rather than an entire native script. Defaults to `false` for ASTs
    /// serialized before this field existed.
    #[serde(default)]
    pub is_native: bool,
    pub imports: Vec<ImportDecl>,
    pub properties: Vec<PropertyDecl>,
    pub variables: Vec<VariableDecl>,
    pub functions: Vec<FunctionDecl>,
    pub states: Vec<StateDecl>,
    /// Fallout 4 only (`GameEdition::Fallout4`): the script's own custom
    /// `Struct .. EndStruct` type declarations. Always empty when parsed
    /// in Skyrim mode.
    #[serde(default)]
    pub structs: Vec<StructDecl>,
    /// Fallout 4 only (`GameEdition::Fallout4`): `Group .. EndGroup`
    /// property groupings. A property declared inside a group appears
    /// here, not in `properties`. Always empty when parsed in Skyrim mode.
    #[serde(default)]
    pub groups: Vec<GroupDecl>,
    /// The line the `ScriptName` keyword itself starts on. Lets downstream
    /// tooling (see `property-sorting` in `papyrus-lints`) locate the
    /// `ScriptName` declaration without re-scanning the original source
    /// text for it.
    pub line: usize,
}

/// A single `Import <ScriptName>` statement, alongside the line it's
/// declared on. Lets downstream tooling (see `unused-import` in
/// `papyrus-lints`) point a diagnostic at the statement itself without
/// re-scanning the original source text for it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportDecl {
    pub name: String,
    pub line: usize,
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
    /// The property's visibility. Unannotated properties use Papyrus's
    /// default public access level.
    #[serde(default)]
    pub access_level: AccessLevel,
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

/// A single member of a Fallout 4 `Struct .. EndStruct` declaration.
/// Struct members carry no `Hidden`/`Conditional` flags -- only a type,
/// name, and optional default value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructMember {
    pub type_name: TypeName,
    pub name: String,
    pub value: Option<Expr>,
    pub line: usize,
}

/// A Fallout 4 only (`GameEdition::Fallout4`) custom `Struct .. EndStruct`
/// type declaration. An instance of the struct is created with `New
/// <StructName>` ([`Expr::NewStruct`]), not `New <StructName>[size]`
/// (array creation, [`Expr::NewArray`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructDecl {
    pub name: String,
    pub members: Vec<StructMember>,
    pub line: usize,
}

/// A Fallout 4 only (`GameEdition::Fallout4`) `Group .. EndGroup` block:
/// purely a Creation Kit organizational aid, wrapping one or more property
/// declarations under a named, optionally-collapsed heading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupDecl {
    pub name: String,
    pub is_collapsed_on_base: bool,
    pub is_collapsed_on_ref: bool,
    pub properties: Vec<PropertyDecl>,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Param {
    pub type_name: TypeName,
    pub name: String,
    pub default: Option<Expr>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessLevel {
    Private,
    Protected,
    #[default]
    Public,
}

/// Build-time metadata describing why a function is deprecated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deprecation {
    pub replacement: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionDecl {
    pub name: String,
    pub return_type: Option<TypeName>,
    pub params: Vec<Param>,
    pub is_global: bool,
    pub is_native: bool,
    pub is_event: bool,
    /// Fallout 4 only (`GameEdition::Fallout4`): the function is compiled
    /// only into debug builds of the game's scripts.
    #[serde(default)]
    pub is_debug_only: bool,
    /// Fallout 4 only (`GameEdition::Fallout4`): the function is compiled
    /// only into beta builds of the game's scripts.
    #[serde(default)]
    pub is_beta_only: bool,
    /// The function's visibility. Unannotated functions use Papyrus's
    /// default public access level.
    #[serde(default)]
    pub access_level: AccessLevel,
    /// Deprecation metadata supplied by a build-time AST producer. Ordinary
    /// parser output leaves this empty.
    #[serde(default)]
    pub deprecation: Option<Deprecation>,
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
        /// The line and column the `Else` keyword itself starts on, when
        /// this `If` has an `Else` clause; `None` when it has none. Both
        /// "no `Else` clause" and "an empty `Else` clause" leave
        /// `else_body` empty, so this is what lets downstream tooling (see
        /// `empty-body` in `papyrus-lints`) tell them apart without
        /// re-lexing the source.
        else_line: Option<usize>,
        else_col: Option<usize>,
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
    /// Fallout 4 only (`GameEdition::Fallout4`): type-check operator
    /// `expr is Type`. The right-hand side is a type name (possibly
    /// colon-qualified), not a value expression. Evaluates to `Bool`.
    Is {
        value: Box<Expr>,
        type_name: String,
    },
    NewArray {
        type_name: TypeName,
        size: Box<Expr>,
    },
    /// Fallout 4 only (`GameEdition::Fallout4`): `New <StructName>`,
    /// creating an instance of a custom `Struct .. EndStruct` type. Unlike
    /// [`Expr::NewArray`], no `[size]` follows the type name.
    NewStruct {
        type_name: String,
    },
}
