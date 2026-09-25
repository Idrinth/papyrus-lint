use super::{PResult, Parser};
use crate::ast::*;
use crate::token::{Keyword, TokenKind};

#[derive(Default)]
struct ScriptFlags {
    is_hidden: bool,
    is_conditional: bool,
    is_native: bool,
}

#[derive(Default)]
struct PropertyFlags {
    is_auto: bool,
    is_auto_read_only: bool,
    is_hidden: bool,
    is_conditional: bool,
    access_level: AccessLevel,
}

#[derive(Default)]
struct FunctionFlags {
    is_global: bool,
    is_native: bool,
    is_debug_only: bool,
    is_beta_only: bool,
    access_level: AccessLevel,
}

impl Parser {
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

        let flags = self.parse_script_flags()?;

        let mut script = Script {
            name,
            extends,
            is_hidden: flags.is_hidden,
            is_conditional: flags.is_conditional,
            is_native: flags.is_native,
            imports: Vec::new(),
            properties: Vec::new(),
            variables: Vec::new(),
            custom_events: Vec::new(),
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

    fn parse_script_flags(&mut self) -> PResult<ScriptFlags> {
        let mut flags = ScriptFlags::default();
        loop {
            if self.at_keyword(Keyword::Hidden) {
                self.advance();
                flags.is_hidden = true;
            } else if self.at_keyword(Keyword::Conditional) {
                self.advance();
                flags.is_conditional = true;
            } else if self.at_keyword(Keyword::Native) {
                self.advance();
                flags.is_native = true;
            } else if self.mode.has_fallout4_dialect()
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
        Ok(flags)
    }

    fn parse_member(&mut self, script: &mut Script) -> PResult<()> {
        if self.mode.has_fallout4_dialect() && self.at_keyword(Keyword::Struct) {
            script.structs.push(self.parse_struct()?);
            return Ok(());
        }

        if self.mode.has_fallout4_dialect() && self.at_keyword(Keyword::Group) {
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

        // Not a reserved word: Skyrim still uses it as a name
        // (`ScriptName CustomEvent`). Fallout 4 and later recognize the
        // declaration here; Skyrim rejects that form instead of the token.
        if self.at_identifier_ignore_ascii_case("CustomEvent") {
            if !self.mode.has_fallout4_dialect() {
                return Err(self.error("CustomEvent is a Fallout 4 and later declaration"));
            }
            let line = self.current().line;
            self.advance();
            let name = self.expect_identifier()?;
            self.expect_terminator()?;
            script.custom_events.push(CustomEventDecl { name, line });
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

    pub(super) fn parse_type_name(&mut self) -> PResult<TypeName> {
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

        let flags = self.parse_property_flags()?;

        if !flags.is_auto && !flags.is_auto_read_only {
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
            is_auto: flags.is_auto,
            is_auto_read_only: flags.is_auto_read_only,
            is_hidden: flags.is_hidden,
            is_conditional: flags.is_conditional,
            access_level: flags.access_level,
            line,
        })
    }

    fn parse_property_flags(&mut self) -> PResult<PropertyFlags> {
        let mut flags = PropertyFlags::default();
        loop {
            if self.at_keyword(Keyword::Auto) {
                self.advance();
                flags.is_auto = true;
            } else if self.at_keyword(Keyword::AutoReadOnly) {
                self.advance();
                flags.is_auto_read_only = true;
            } else if self.at_keyword(Keyword::Hidden) {
                self.advance();
                flags.is_hidden = true;
            } else if self.at_keyword(Keyword::Conditional) {
                self.advance();
                flags.is_conditional = true;
            } else if self.mode.has_fallout4_dialect()
                && (self.at_identifier_ignore_ascii_case("Const")
                    || self.at_identifier_ignore_ascii_case("Mandatory"))
            {
                self.advance();
            } else if matches!(self.kind(), TokenKind::CommentAnnotation(_)) {
                flags.access_level = self.parse_access_level()?;
            } else {
                break;
            }
        }
        self.expect_terminator()?;
        Ok(flags)
    }

    /// Fallout 4 only: `Struct <Name>` .. `EndStruct`, a block of typed
    /// member declarations with optional default values. Only called in
    /// Fallout 4 / Starfield mode.
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
            let member_name = self.expect_struct_member_name()?;
            let mut value = None;
            if matches!(self.kind(), TokenKind::Assign) {
                self.advance();
                value = Some(self.parse_expr()?);
            }
            while self.at_keyword(Keyword::Hidden)
                || self.at_keyword(Keyword::Conditional)
                || self.at_identifier_ignore_ascii_case("Const")
            {
                self.advance();
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

    /// Fallout 4 only: `Group <Name> [Collapsed|CollapsedOnBase]
    /// [CollapsedOnRef]` .. `EndGroup`, a block of property declarations.
    /// Only called in Fallout 4 / Starfield mode.
    fn parse_group(&mut self) -> PResult<GroupDecl> {
        let line = self.current().line;
        self.expect_keyword(Keyword::Group)?;
        let name = self.expect_identifier()?;

        let mut is_collapsed_on_base = false;
        let mut is_collapsed_on_ref = false;
        loop {
            if self.at_keyword(Keyword::Collapsed) {
                self.advance();
                is_collapsed_on_base = true;
                is_collapsed_on_ref = true;
            } else if self.at_keyword(Keyword::CollapsedOnBase) {
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

    pub(super) fn parse_variable_tail(
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
            } else if self.mode.has_fallout4_dialect()
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
        if is_event && self.mode.has_fallout4_dialect() && matches!(self.kind(), TokenKind::Dot) {
            self.advance();
            name.push('.');
            name.push_str(&self.expect_identifier()?);
        }
        self.expect(TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.expect(TokenKind::RParen)?;

        let flags = self.parse_function_flags()?;

        let mut body = Vec::new();
        if !flags.is_native {
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
            is_global: flags.is_global,
            is_native: flags.is_native,
            is_event,
            is_debug_only: flags.is_debug_only,
            is_beta_only: flags.is_beta_only,
            access_level: flags.access_level,
            deprecation: None,
            body,
            line,
            state: None,
        })
    }

    fn parse_function_flags(&mut self) -> PResult<FunctionFlags> {
        let mut flags = FunctionFlags::default();
        loop {
            if self.at_keyword(Keyword::Global) {
                self.advance();
                flags.is_global = true;
            } else if self.at_keyword(Keyword::Native) {
                self.advance();
                flags.is_native = true;
            } else if self.mode.has_fallout4_dialect() && self.at_keyword(Keyword::DebugOnly) {
                self.advance();
                flags.is_debug_only = true;
            } else if self.mode.has_fallout4_dialect() && self.at_keyword(Keyword::BetaOnly) {
                self.advance();
                flags.is_beta_only = true;
            } else if self.mode.has_starfield_dialect() && self.at_starfield_access_flag() {
                flags.access_level = self.parse_starfield_access_flag();
            } else if matches!(self.kind(), TokenKind::CommentAnnotation(_)) {
                flags.access_level = self.parse_access_level()?;
            } else {
                break;
            }
        }
        self.expect_terminator()?;
        Ok(flags)
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

    /// Starfield writes access the same way `; @private` / `; @protected`
    /// do, but as header flags (`Private`, `Protected`, `SelfOnly`,
    /// `Internal`) instead of comment annotations. `SelfOnly` is the
    /// native-header companion to `Protected` (`native protected selfonly`
    /// on `ScriptObject`); `Internal` is the script-local companion to
    /// `Private`.
    fn at_starfield_access_flag(&self) -> bool {
        ["private", "protected", "selfonly", "internal"]
            .iter()
            .any(|flag| self.at_identifier_ignore_ascii_case(flag))
    }

    fn parse_starfield_access_flag(&mut self) -> AccessLevel {
        let TokenKind::Identifier(name) = self.kind().clone() else {
            unreachable!("parse_starfield_access_flag is only called for an identifier flag");
        };
        self.advance();
        match name.to_ascii_lowercase().as_str() {
            "private" | "internal" => AccessLevel::Private,
            "protected" | "selfonly" => AccessLevel::Protected,
            _ => unreachable!("at_starfield_access_flag already filtered the spelling"),
        }
    }

    fn parse_params(&mut self) -> PResult<Vec<Param>> {
        let mut params = Vec::new();
        if matches!(self.kind(), TokenKind::RParen) {
            return Ok(params);
        }
        loop {
            let type_name = self.parse_type_name()?;
            let name = self.expect_value_identifier()?;
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
}
