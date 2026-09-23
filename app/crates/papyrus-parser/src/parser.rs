//! Recursive-descent parser producing an AST from a token stream.

use super::ast::*;
use super::token::{Keyword, Token, TokenKind};

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.col, self.message)
    }
}

type PResult<T> = Result<T, ParseError>;

/// Which game's Papyrus dialect a [`Parser`] accepts. Skyrim is the
/// original language `papyrus-parser` was built for; Fallout 4 adds a
/// handful of new constructs (custom `Struct`s, property `Group`s, the
/// `DebugOnly`/`BetaOnly` script and function flags, and colon-qualified names such
/// as `DLC03:Foo` on types, `extends`, `new`, and calls) on top of it. A construct that's
/// Fallout 4 only is rejected the same way an unrecognized token always
/// is -- as an ordinary [`ParseError`] -- when parsed in [`Self::Skyrim`]
/// mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GameEdition {
    #[default]
    Skyrim,
    Fallout4,
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    mode: GameEdition,
}

impl Parser {
    /// Creates a parser over a non-empty token stream, accepting Skyrim's
    /// Papyrus dialect. See [`Self::new_with_mode`] to parse Fallout 4's.
    ///
    /// # Panics
    ///
    /// Panics when `tokens` is empty. Lexer-produced streams always contain
    /// at least the final [`TokenKind::Eof`] token.
    pub fn new(tokens: Vec<Token>) -> Self {
        Self::new_with_mode(tokens, GameEdition::default())
    }

    /// Same as [`Self::new`], but accepting `mode`'s Papyrus dialect.
    ///
    /// # Panics
    ///
    /// Panics when `tokens` is empty. Lexer-produced streams always contain
    /// at least the final [`TokenKind::Eof`] token.
    pub fn new_with_mode(tokens: Vec<Token>, mode: GameEdition) -> Self {
        assert!(!tokens.is_empty(), "parser token stream must not be empty");
        Parser {
            tokens,
            pos: 0,
            mode,
        }
    }

    fn current(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn kind(&self) -> &TokenKind {
        &self.current().kind
    }

    fn peek_kind(&self, offset: usize) -> &TokenKind {
        let idx = (self.pos + offset).min(self.tokens.len() - 1);
        &self.tokens[idx].kind
    }

    fn is_eof(&self) -> bool {
        matches!(self.kind(), TokenKind::Eof)
    }

    fn advance(&mut self) -> Token {
        let tok = self.current().clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        tok
    }

    fn error(&self, message: impl Into<String>) -> ParseError {
        let tok = self.current();
        ParseError {
            message: message.into(),
            line: tok.line,
            col: tok.col,
        }
    }

    fn skip_newlines(&mut self) {
        loop {
            if matches!(self.kind(), TokenKind::Newline) {
                self.advance();
            } else if matches!(self.kind(), TokenKind::CommentAnnotation(_)) {
                // `@public` / `@protected` / `@private` are tokens so a
                // declaration header can record them. On a comment line of
                // their own they are not syntax; rejecting them used to
                // discard the whole script (#1180).
                self.advance();
            } else {
                break;
            }
        }
    }

    /// Consumes a single statement terminator (newline or end of file).
    ///
    /// Access-level annotations that ride along on the same line but were
    /// not consumed as a function or property flag are comments, not code.
    fn expect_terminator(&mut self) -> PResult<()> {
        while matches!(self.kind(), TokenKind::CommentAnnotation(_)) {
            self.advance();
        }
        if self.is_eof() {
            return Ok(());
        }
        if matches!(self.kind(), TokenKind::Newline) {
            self.advance();
            return Ok(());
        }
        Err(self.error(format!("expected end of line, found {:?}", self.kind())))
    }

    fn expect_keyword(&mut self, kw: Keyword) -> PResult<Token> {
        if matches!(self.kind(), TokenKind::Keyword(k) if *k == kw) {
            Ok(self.advance())
        } else {
            Err(self.error(format!(
                "expected keyword {:?}, found {:?}",
                kw,
                self.kind()
            )))
        }
    }

    fn at_keyword(&self, kw: Keyword) -> bool {
        matches!(self.kind(), TokenKind::Keyword(k) if *k == kw)
    }

    fn at_identifier_ignore_ascii_case(&self, expected: &str) -> bool {
        matches!(self.kind(), TokenKind::Identifier(name) if name.eq_ignore_ascii_case(expected))
    }

    fn expect_identifier(&mut self) -> PResult<String> {
        match self.kind().clone() {
            TokenKind::Identifier(name) => {
                self.advance();
                Ok(name)
            }
            other => Err(self.error(format!("expected identifier, found {:?}", other))),
        }
    }

    /// Appends any immediately following `:Segment` pieces onto `name`.
    ///
    /// Fallout 4 uses colon-qualified names (`DLC03:Foo`,
    /// `InstanceData:Owner`, including further segments) for scripts,
    /// types, `new`, and function names. The colon is not an operator.
    fn append_colon_segments(&mut self, mut name: String) -> PResult<String> {
        while matches!(self.kind(), TokenKind::Colon) {
            self.advance();
            name.push(':');
            name.push_str(&self.expect_identifier()?);
        }
        Ok(name)
    }

    /// Parses a script name. Fallout 4 allows colon-qualified names such
    /// as `User:MyQuestScript`; Skyrim script names stay a single
    /// identifier, so a `:` is still unexpected there.
    fn expect_script_name(&mut self) -> PResult<String> {
        self.expect_qualified_name()
    }

    /// An identifier, or in [`GameEdition::Fallout4`] only a
    /// colon-qualified name (`Namespace:Name`, including further segments).
    fn expect_qualified_name(&mut self) -> PResult<String> {
        let name = self.expect_identifier()?;
        if self.mode == GameEdition::Fallout4 {
            self.append_colon_segments(name)
        } else {
            Ok(name)
        }
    }

    /// Like `expect_identifier`, but also accepts the `Length` keyword,
    /// which is only ever meaningful as an array's `.Length` property.
    fn expect_property_name(&mut self) -> PResult<String> {
        if matches!(self.kind(), TokenKind::Keyword(Keyword::Length)) {
            self.advance();
            return Ok("Length".to_string());
        }
        self.expect_identifier()
    }

    fn expect(&mut self, kind: TokenKind) -> PResult<Token> {
        if *self.kind() == kind {
            Ok(self.advance())
        } else {
            Err(self.error(format!("expected {:?}, found {:?}", kind, self.kind())))
        }
    }

    // ---- top level -----------------------------------------------------

    pub fn parse_script(&mut self) -> PResult<Script> {
        self.skip_newlines();
        let line = self.current().line;
        self.expect_keyword(Keyword::ScriptName)?;
        let name = self.expect_script_name()?;

        let mut extends = None;
        if self.at_keyword(Keyword::Extends) {
            self.advance();
            extends = Some(self.expect_qualified_name()?);
        }

        let mut is_hidden = false;
        let mut is_conditional = false;
        let mut is_native = false;
        loop {
            if self.at_keyword(Keyword::Hidden) {
                self.advance();
                is_hidden = true;
            } else if self.at_keyword(Keyword::Conditional) {
                self.advance();
                is_conditional = true;
            } else if self.at_keyword(Keyword::Native) {
                // A whole script implemented natively by the engine (e.g.
                // Fallout 4's own `Actor.psc extends ObjectReference
                // Native Hidden`) rather than one native function inside
                // it. Accepted in any dialect: Skyrim's own vanilla
                // archive never uses it, but nothing about the flag
                // itself is Fallout 4 specific.
                self.advance();
                is_native = true;
            } else if self.mode == GameEdition::Fallout4
                && (self.at_keyword(Keyword::DebugOnly)
                    || self.at_keyword(Keyword::BetaOnly)
                    || self.at_identifier_ignore_ascii_case("Const")
                    || self.at_identifier_ignore_ascii_case("Default"))
            {
                self.advance();
            } else {
                break;
            }
        }
        self.expect_terminator()?;

        let mut script = Script {
            name,
            extends,
            is_hidden,
            is_conditional,
            is_native,
            imports: Vec::new(),
            properties: Vec::new(),
            variables: Vec::new(),
            functions: Vec::new(),
            states: Vec::new(),
            structs: Vec::new(),
            groups: Vec::new(),
            line,
        };

        loop {
            self.skip_newlines();
            if self.is_eof() {
                break;
            }
            self.parse_member(&mut script)?;
        }

        Ok(script)
    }

    fn parse_member(&mut self, script: &mut Script) -> PResult<()> {
        if self.mode == GameEdition::Fallout4 && self.at_keyword(Keyword::Struct) {
            script.structs.push(self.parse_struct()?);
            return Ok(());
        }

        if self.mode == GameEdition::Fallout4 && self.at_keyword(Keyword::Group) {
            script.groups.push(self.parse_group()?);
            return Ok(());
        }

        if self.at_keyword(Keyword::Import) {
            let line = self.current().line;
            self.advance();
            let name = self.expect_qualified_name()?;
            self.expect_terminator()?;
            script.imports.push(ImportDecl { name, line });
            return Ok(());
        }

        if self.at_keyword(Keyword::State) {
            script.states.push(self.parse_state(false)?);
            return Ok(());
        }

        if self.at_keyword(Keyword::Auto) {
            self.advance();
            script.states.push(self.parse_state(true)?);
            return Ok(());
        }

        if self.at_keyword(Keyword::Function) {
            script.functions.push(self.parse_function(None, false)?);
            return Ok(());
        }

        if self.at_keyword(Keyword::Event) {
            script.functions.push(self.parse_function(None, true)?);
            return Ok(());
        }

        let line = self.current().line;
        let type_name = self.parse_type_name()?;

        if self.at_keyword(Keyword::Function) {
            script
                .functions
                .push(self.parse_function(Some(type_name), false)?);
            return Ok(());
        }

        if self.at_keyword(Keyword::Property) {
            script
                .properties
                .push(self.parse_property(type_name, line)?);
            return Ok(());
        }

        let name = self.expect_identifier()?;
        script
            .variables
            .push(self.parse_variable_tail(type_name, name, line)?);
        Ok(())
    }

    fn parse_type_name(&mut self) -> PResult<TypeName> {
        let name = self.expect_qualified_name()?;
        let mut is_array = false;
        if matches!(self.kind(), TokenKind::LBracket) {
            self.advance();
            self.expect(TokenKind::RBracket)?;
            is_array = true;
        }
        Ok(TypeName { name, is_array })
    }

    fn parse_property(&mut self, type_name: TypeName, line: usize) -> PResult<PropertyDecl> {
        self.expect_keyword(Keyword::Property)?;
        let name = self.expect_identifier()?;

        let mut value = None;
        if matches!(self.kind(), TokenKind::Assign) {
            self.advance();
            value = Some(self.parse_expr()?);
        }

        let mut is_auto = false;
        let mut is_auto_read_only = false;
        let mut is_hidden = false;
        let mut is_conditional = false;
        let mut access_level = AccessLevel::default();
        loop {
            if self.at_keyword(Keyword::Auto) {
                self.advance();
                is_auto = true;
            } else if self.at_keyword(Keyword::AutoReadOnly) {
                self.advance();
                is_auto_read_only = true;
            } else if self.at_keyword(Keyword::Hidden) {
                self.advance();
                is_hidden = true;
            } else if self.at_keyword(Keyword::Conditional) {
                self.advance();
                is_conditional = true;
            } else if self.mode == GameEdition::Fallout4
                && (self.at_identifier_ignore_ascii_case("Const")
                    || self.at_identifier_ignore_ascii_case("Mandatory"))
            {
                self.advance();
            } else if matches!(self.kind(), TokenKind::CommentAnnotation(_)) {
                access_level = self.parse_access_level()?;
            } else {
                break;
            }
        }
        self.expect_terminator()?;

        if !is_auto && !is_auto_read_only {
            // Full property: skip the Function/EndFunction get/set block(s);
            // parsing their bodies is out of scope for the basic AST.
            while !self.at_keyword(Keyword::EndProperty) && !self.is_eof() {
                self.advance();
            }
            self.expect_keyword(Keyword::EndProperty)?;
            self.expect_terminator()?;
        }

        Ok(PropertyDecl {
            type_name,
            name,
            value,
            is_auto,
            is_auto_read_only,
            is_hidden,
            is_conditional,
            access_level,
            line,
        })
    }

    /// Fallout 4 only: `Struct <Name>` .. `EndStruct`, a block of typed
    /// member declarations with optional default values. Only called in
    /// [`GameEdition::Fallout4`] mode.
    fn parse_struct(&mut self) -> PResult<StructDecl> {
        let line = self.current().line;
        self.expect_keyword(Keyword::Struct)?;
        let name = self.expect_identifier()?;
        self.expect_terminator()?;

        let mut members = Vec::new();
        loop {
            self.skip_newlines();
            if self.at_keyword(Keyword::EndStruct) {
                break;
            }
            if self.is_eof() {
                return Err(self.error("expected EndStruct, found end of file"));
            }
            let member_line = self.current().line;
            let type_name = self.parse_type_name()?;
            let member_name = self.expect_identifier()?;
            let mut value = None;
            if matches!(self.kind(), TokenKind::Assign) {
                self.advance();
                value = Some(self.parse_expr()?);
            }
            self.expect_terminator()?;
            members.push(StructMember {
                type_name,
                name: member_name,
                value,
                line: member_line,
            });
        }
        self.expect_keyword(Keyword::EndStruct)?;
        self.expect_terminator()?;

        Ok(StructDecl {
            name,
            members,
            line,
        })
    }

    /// Fallout 4 only: `Group <Name> [CollapsedOnBase] [CollapsedOnRef]` ..
    /// `EndGroup`, a block of property declarations. Only called in
    /// [`GameEdition::Fallout4`] mode.
    fn parse_group(&mut self) -> PResult<GroupDecl> {
        let line = self.current().line;
        self.expect_keyword(Keyword::Group)?;
        let name = self.expect_identifier()?;

        let mut is_collapsed_on_base = false;
        let mut is_collapsed_on_ref = false;
        loop {
            if self.at_keyword(Keyword::CollapsedOnBase) {
                self.advance();
                is_collapsed_on_base = true;
            } else if self.at_keyword(Keyword::CollapsedOnRef) {
                self.advance();
                is_collapsed_on_ref = true;
            } else {
                break;
            }
        }
        self.expect_terminator()?;

        let mut properties = Vec::new();
        loop {
            self.skip_newlines();
            if self.at_keyword(Keyword::EndGroup) {
                break;
            }
            if self.is_eof() {
                return Err(self.error("expected EndGroup, found end of file"));
            }
            let prop_line = self.current().line;
            let type_name = self.parse_type_name()?;
            if !self.at_keyword(Keyword::Property) {
                return Err(self.error(format!(
                    "expected Property declaration inside Group, found {:?}",
                    self.kind()
                )));
            }
            properties.push(self.parse_property(type_name, prop_line)?);
        }
        self.expect_keyword(Keyword::EndGroup)?;
        self.expect_terminator()?;

        Ok(GroupDecl {
            name,
            is_collapsed_on_base,
            is_collapsed_on_ref,
            properties,
            line,
        })
    }

    fn parse_variable_tail(
        &mut self,
        type_name: TypeName,
        name: String,
        line: usize,
    ) -> PResult<VariableDecl> {
        let mut value = None;
        if matches!(self.kind(), TokenKind::Assign) {
            self.advance();
            value = Some(self.parse_expr()?);
        }
        let mut is_conditional = false;
        loop {
            if self.at_keyword(Keyword::Conditional) {
                self.advance();
                is_conditional = true;
            } else if self.mode == GameEdition::Fallout4
                && self.at_identifier_ignore_ascii_case("Const")
            {
                self.advance();
            } else {
                break;
            }
        }
        self.expect_terminator()?;
        Ok(VariableDecl {
            type_name,
            name,
            value,
            is_conditional,
            line,
        })
    }

    fn parse_state(&mut self, is_auto: bool) -> PResult<StateDecl> {
        let line = self.current().line;
        self.expect_keyword(Keyword::State)?;
        let name = self.expect_identifier()?;
        self.expect_terminator()?;

        let mut functions = Vec::new();
        loop {
            self.skip_newlines();
            if self.at_keyword(Keyword::EndState) {
                break;
            }
            if self.is_eof() {
                return Err(self.error("expected EndState, found end of file"));
            }
            if self.at_keyword(Keyword::Function) {
                functions.push(self.parse_function(None, false)?);
                continue;
            }
            if self.at_keyword(Keyword::Event) {
                functions.push(self.parse_function(None, true)?);
                continue;
            }
            let return_type = self.parse_type_name()?;
            functions.push(self.parse_function(Some(return_type), false)?);
        }
        self.expect_keyword(Keyword::EndState)?;
        self.expect_terminator()?;

        for function in &mut functions {
            function.state = Some(name.clone());
        }

        Ok(StateDecl {
            name,
            is_auto,
            functions,
            line,
        })
    }

    fn parse_function(
        &mut self,
        return_type: Option<TypeName>,
        is_event: bool,
    ) -> PResult<FunctionDecl> {
        let line = self.current().line;
        if is_event {
            self.expect_keyword(Keyword::Event)?;
        } else {
            self.expect_keyword(Keyword::Function)?;
        }
        let mut name = self.expect_qualified_name()?;
        // Fallout 4 remote / custom events are declared as
        // `Event <Script>.<EventName>(...)`, optionally with a
        // colon-qualified script (`Event DLC03:Foo.Bar(...)`). Skyrim
        // events are a bare identifier; a `.` there is still
        // `expected LParen, found Dot`.
        if is_event && self.mode == GameEdition::Fallout4 && matches!(self.kind(), TokenKind::Dot) {
            self.advance();
            name.push('.');
            name.push_str(&self.expect_identifier()?);
        }
        self.expect(TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.expect(TokenKind::RParen)?;

        let mut is_global = false;
        let mut is_native = false;
        let mut is_debug_only = false;
        let mut is_beta_only = false;
        let mut access_level = AccessLevel::default();
        loop {
            if self.at_keyword(Keyword::Global) {
                self.advance();
                is_global = true;
            } else if self.at_keyword(Keyword::Native) {
                self.advance();
                is_native = true;
            } else if self.mode == GameEdition::Fallout4 && self.at_keyword(Keyword::DebugOnly) {
                self.advance();
                is_debug_only = true;
            } else if self.mode == GameEdition::Fallout4 && self.at_keyword(Keyword::BetaOnly) {
                self.advance();
                is_beta_only = true;
            } else if matches!(self.kind(), TokenKind::CommentAnnotation(_)) {
                access_level = self.parse_access_level()?;
            } else {
                break;
            }
        }
        self.expect_terminator()?;

        let mut body = Vec::new();
        if !is_native {
            let end_kw = if is_event {
                Keyword::EndEvent
            } else {
                Keyword::EndFunction
            };
            body = self.parse_block(&[end_kw])?;
            self.expect_keyword(end_kw)?;
            self.expect_terminator()?;
        }

        Ok(FunctionDecl {
            name,
            return_type,
            params,
            is_global,
            is_native,
            is_event,
            is_debug_only,
            is_beta_only,
            access_level,
            deprecation: None,
            body,
            line,
            state: None,
        })
    }

    fn parse_access_level(&mut self) -> PResult<AccessLevel> {
        let TokenKind::CommentAnnotation(annotation) = self.advance().kind else {
            unreachable!("parse_access_level is only called for a comment annotation");
        };
        match annotation.to_ascii_lowercase().as_str() {
            "public" => Ok(AccessLevel::Public),
            "protected" => Ok(AccessLevel::Protected),
            "private" => Ok(AccessLevel::Private),
            _ => unreachable!("the lexer only emits supported access level annotations"),
        }
    }

    fn parse_params(&mut self) -> PResult<Vec<Param>> {
        let mut params = Vec::new();
        if matches!(self.kind(), TokenKind::RParen) {
            return Ok(params);
        }
        loop {
            let type_name = self.parse_type_name()?;
            let name = self.expect_identifier()?;
            let mut default = None;
            if matches!(self.kind(), TokenKind::Assign) {
                self.advance();
                default = Some(self.parse_expr()?);
            }
            params.push(Param {
                type_name,
                name,
                default,
            });
            if matches!(self.kind(), TokenKind::Comma) {
                self.advance();
                continue;
            }
            break;
        }
        Ok(params)
    }

    // ---- statements ------------------------------------------------------

    fn parse_block(&mut self, end_keywords: &[Keyword]) -> PResult<Vec<Stmt>> {
        let mut stmts = Vec::new();
        loop {
            self.skip_newlines();
            if self.is_eof() {
                break;
            }
            if end_keywords.iter().any(|kw| self.at_keyword(*kw)) {
                break;
            }
            stmts.push(self.parse_stmt()?);
        }
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> PResult<Stmt> {
        let line = self.current().line;

        if self.at_keyword(Keyword::If) {
            return self.parse_if();
        }
        if self.at_keyword(Keyword::While) {
            return self.parse_while();
        }
        if self.at_keyword(Keyword::Return) {
            self.advance();
            let value = if matches!(
                self.kind(),
                TokenKind::Newline | TokenKind::CommentAnnotation(_)
            ) || self.is_eof()
            {
                None
            } else {
                Some(self.parse_expr()?)
            };
            self.expect_terminator()?;
            return Ok(Stmt::Return { value, line });
        }

        if self.looks_like_var_decl() {
            let type_name = self.parse_type_name()?;
            let name = self.expect_identifier()?;
            return Ok(Stmt::VarDecl(
                self.parse_variable_tail(type_name, name, line)?,
            ));
        }

        let target = self.parse_expr()?;
        let op = match self.kind() {
            TokenKind::Assign => Some(AssignOp::Assign),
            TokenKind::PlusAssign => Some(AssignOp::AddAssign),
            TokenKind::MinusAssign => Some(AssignOp::SubAssign),
            TokenKind::StarAssign => Some(AssignOp::MulAssign),
            TokenKind::SlashAssign => Some(AssignOp::DivAssign),
            TokenKind::PercentAssign => Some(AssignOp::ModAssign),
            _ => None,
        };
        if let Some(op) = op {
            self.advance();
            let value = self.parse_expr()?;
            self.expect_terminator()?;
            return Ok(Stmt::Assign {
                target,
                op,
                value,
                line,
            });
        }

        self.expect_terminator()?;
        Ok(Stmt::Expr {
            value: target,
            line,
        })
    }

    /// Disambiguates a local variable declaration (`Type name = ...`) from an
    /// expression statement / assignment by looking ahead for the
    /// `Identifier [ ':' Identifier ]* [ '[' ']' ] Identifier` pattern,
    /// without consuming tokens. Colon segments are part of a type name
    /// only in [`GameEdition::Fallout4`] mode.
    fn looks_like_var_decl(&self) -> bool {
        let mut i = self.pos;
        if !matches!(
            self.tokens.get(i).map(|t| &t.kind),
            Some(TokenKind::Identifier(_))
        ) {
            return false;
        }
        i += 1;
        if self.mode == GameEdition::Fallout4 {
            while matches!(self.tokens.get(i).map(|t| &t.kind), Some(TokenKind::Colon))
                && matches!(
                    self.tokens.get(i + 1).map(|t| &t.kind),
                    Some(TokenKind::Identifier(_))
                )
            {
                i += 2;
            }
        }
        if matches!(
            self.tokens.get(i).map(|t| &t.kind),
            Some(TokenKind::LBracket)
        ) && matches!(
            self.tokens.get(i + 1).map(|t| &t.kind),
            Some(TokenKind::RBracket)
        ) {
            i += 2;
        }
        matches!(
            self.tokens.get(i).map(|t| &t.kind),
            Some(TokenKind::Identifier(_))
        )
    }

    fn parse_if(&mut self) -> PResult<Stmt> {
        let line = self.current().line;
        let col = self.current().col;
        self.expect_keyword(Keyword::If)?;
        let mut branches = Vec::new();
        let condition = self.parse_expr()?;
        self.expect_terminator()?;
        let body = self.parse_block(&[Keyword::ElseIf, Keyword::Else, Keyword::EndIf])?;
        branches.push(IfBranch {
            condition,
            body,
            line,
            col,
        });

        while self.at_keyword(Keyword::ElseIf) {
            let line = self.current().line;
            let col = self.current().col;
            self.advance();
            let condition = self.parse_expr()?;
            self.expect_terminator()?;
            let body = self.parse_block(&[Keyword::ElseIf, Keyword::Else, Keyword::EndIf])?;
            branches.push(IfBranch {
                condition,
                body,
                line,
                col,
            });
        }

        let (else_body, else_line, else_col) = if self.at_keyword(Keyword::Else) {
            let else_line = self.current().line;
            let else_col = self.current().col;
            self.advance();
            self.expect_terminator()?;
            let body = self.parse_block(&[Keyword::EndIf])?;
            (body, Some(else_line), Some(else_col))
        } else {
            (Vec::new(), None, None)
        };

        self.expect_keyword(Keyword::EndIf)?;
        self.expect_terminator()?;

        Ok(Stmt::If {
            branches,
            else_body,
            else_line,
            else_col,
            line,
        })
    }

    fn parse_while(&mut self) -> PResult<Stmt> {
        let line = self.current().line;
        let col = self.current().col;
        self.expect_keyword(Keyword::While)?;
        let condition = self.parse_expr()?;
        self.expect_terminator()?;
        let body = self.parse_block(&[Keyword::EndWhile])?;
        self.expect_keyword(Keyword::EndWhile)?;
        self.expect_terminator()?;
        Ok(Stmt::While {
            condition,
            body,
            line,
            col,
        })
    }

    // ---- expressions -------------------------------------------------

    pub fn parse_expr(&mut self) -> PResult<Expr> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> PResult<Expr> {
        let mut left = self.parse_and()?;
        while matches!(self.kind(), TokenKind::OrOr) {
            self.advance();
            let right = self.parse_and()?;
            left = Expr::Binary {
                left: Box::new(left),
                op: BinaryOp::Or,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> PResult<Expr> {
        let mut left = self.parse_equality()?;
        while matches!(self.kind(), TokenKind::AndAnd) {
            self.advance();
            let right = self.parse_equality()?;
            left = Expr::Binary {
                left: Box::new(left),
                op: BinaryOp::And,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> PResult<Expr> {
        let mut left = self.parse_comparison()?;
        loop {
            let op = match self.kind() {
                TokenKind::Eq => BinaryOp::Eq,
                TokenKind::NotEq => BinaryOp::NotEq,
                _ => break,
            };
            self.advance();
            let right = self.parse_comparison()?;
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> PResult<Expr> {
        let mut left = self.parse_additive()?;
        loop {
            let op = match self.kind() {
                TokenKind::Gt => BinaryOp::Gt,
                TokenKind::Lt => BinaryOp::Lt,
                TokenKind::GtEq => BinaryOp::GtEq,
                TokenKind::LtEq => BinaryOp::LtEq,
                _ => break,
            };
            self.advance();
            let right = self.parse_additive()?;
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> PResult<Expr> {
        let mut left = self.parse_multiplicative()?;
        loop {
            let op = match self.kind() {
                TokenKind::Plus => BinaryOp::Add,
                TokenKind::Minus => BinaryOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplicative()?;
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> PResult<Expr> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.kind() {
                TokenKind::Star => BinaryOp::Mul,
                TokenKind::Slash => BinaryOp::Div,
                TokenKind::Percent => BinaryOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> PResult<Expr> {
        let op = match self.kind() {
            TokenKind::Minus => Some(UnaryOp::Neg),
            TokenKind::Not => Some(UnaryOp::Not),
            _ => None,
        };
        if let Some(op) = op {
            self.advance();
            let operand = self.parse_unary()?;
            return Ok(Expr::Unary {
                op,
                operand: Box::new(operand),
            });
        }
        self.parse_cast()
    }

    fn parse_cast(&mut self) -> PResult<Expr> {
        let mut left = self.parse_postfix()?;
        while self.at_keyword(Keyword::As) {
            self.advance();
            let type_name = self.expect_qualified_name()?;
            left = Expr::Cast {
                value: Box::new(left),
                type_name,
            };
        }
        Ok(left)
    }

    fn parse_postfix(&mut self) -> PResult<Expr> {
        let mut expr = self.parse_primary()?;
        loop {
            match self.kind() {
                TokenKind::Dot => {
                    self.advance();
                    let property = self.expect_property_name()?;
                    expr = Expr::Member {
                        object: Box::new(expr),
                        property,
                    };
                }
                TokenKind::LBracket => {
                    self.advance();
                    let index = self.parse_expr()?;
                    self.expect(TokenKind::RBracket)?;
                    expr = Expr::Index {
                        object: Box::new(expr),
                        index: Box::new(index),
                    };
                }
                TokenKind::LParen => {
                    let line = self.current().line;
                    let col = self.current().col;
                    self.advance();
                    let args = self.parse_args()?;
                    self.expect(TokenKind::RParen)?;
                    expr = Expr::Call {
                        callee: Box::new(expr),
                        args,
                        line,
                        col,
                    };
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_args(&mut self) -> PResult<Vec<Expr>> {
        let mut args = Vec::new();
        if matches!(self.kind(), TokenKind::RParen) {
            return Ok(args);
        }
        loop {
            args.push(self.parse_arg()?);
            if matches!(self.kind(), TokenKind::Comma) {
                self.advance();
                continue;
            }
            break;
        }
        Ok(args)
    }

    /// Parses one call argument: either a plain expression, or a named
    /// argument (`name = value`), Papyrus's syntax for passing an argument
    /// by parameter name instead of by position.
    fn parse_arg(&mut self) -> PResult<Expr> {
        if let TokenKind::Identifier(name) = self.kind().clone() {
            if matches!(self.peek_kind(1), TokenKind::Assign) {
                self.advance();
                self.advance();
                let value = self.parse_expr()?;
                return Ok(Expr::NamedArg {
                    name,
                    value: Box::new(value),
                });
            }
        }
        self.parse_expr()
    }

    fn parse_primary(&mut self) -> PResult<Expr> {
        let tok = self.current().clone();
        match tok.kind {
            TokenKind::IntLiteral(v, format) => {
                self.advance();
                Ok(Expr::Literal(Literal::Int { value: v, format }))
            }
            TokenKind::FloatLiteral(v) => {
                self.advance();
                Ok(Expr::Literal(Literal::Float(v)))
            }
            TokenKind::StringLiteral(ref v) => {
                let v = v.clone();
                self.advance();
                Ok(Expr::Literal(Literal::String(v)))
            }
            TokenKind::Keyword(Keyword::True) => {
                self.advance();
                Ok(Expr::Literal(Literal::Bool(true)))
            }
            TokenKind::Keyword(Keyword::False) => {
                self.advance();
                Ok(Expr::Literal(Literal::Bool(false)))
            }
            TokenKind::Keyword(Keyword::None) => {
                self.advance();
                Ok(Expr::Literal(Literal::None))
            }
            TokenKind::Keyword(Keyword::Self_) => {
                self.advance();
                Ok(Expr::Self_)
            }
            TokenKind::Keyword(Keyword::Parent) => {
                self.advance();
                Ok(Expr::Parent)
            }
            TokenKind::Keyword(Keyword::New) => {
                self.advance();
                let name = self.expect_qualified_name()?;
                if self.mode == GameEdition::Fallout4 && !matches!(self.kind(), TokenKind::LBracket)
                {
                    // Fallout 4 only: `New <StructName>`, creating a struct
                    // instance rather than an array.
                    return Ok(Expr::NewStruct { type_name: name });
                }
                self.expect(TokenKind::LBracket)?;
                let size = self.parse_expr()?;
                self.expect(TokenKind::RBracket)?;
                Ok(Expr::NewArray {
                    type_name: TypeName {
                        name,
                        is_array: false,
                    },
                    size: Box::new(size),
                })
            }
            TokenKind::Identifier(ref name) => {
                let name = name.clone();
                self.advance();
                let name = if self.mode == GameEdition::Fallout4 {
                    self.append_colon_segments(name)?
                } else {
                    name
                };
                Ok(Expr::Identifier(name))
            }
            TokenKind::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(TokenKind::RParen)?;
                Ok(expr)
            }
            other => Err(self.error(format!("unexpected token {:?}", other))),
        }
    }
}
