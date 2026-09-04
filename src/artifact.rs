//! Typed `.lemmaspec` artifacts and deterministic evaluation.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::Serialize;

use crate::ast::{Atom, Clause, CmpOp, Expr, Lit};
use crate::eval::Support;
use crate::{parse_program, Ann, Engine, Term, Value};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueType {
    Symbol,
    Integer,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(untagged)]
pub enum FactValue {
    Symbol(String),
    Integer(i64),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RelationDecl {
    pub name: String,
    pub args: Vec<ValueType>,
    /// Human names for each argument position, e.g. `[mutation, policy]`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<String>,
    /// Sentence template over the roles, e.g. `"{mutation} is governed by {policy}"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reads: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FactDecl {
    pub id: String,
    pub relation: String,
    pub args: Vec<FactValue>,
    pub confidence: f64,
    pub provenance: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuleDecl {
    pub id: String,
    pub derive: String,
    pub when: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub condition_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

impl RuleDecl {
    pub(crate) fn condition_id(&self, index: usize) -> Option<&str> {
        self.condition_ids.get(index).map(String::as_str)
    }

    pub(crate) fn remove_condition(&mut self, index: usize) {
        self.when.remove(index);
        if !self.condition_ids.is_empty() {
            self.condition_ids.remove(index);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExpectationDecl {
    pub id: String,
    pub query: String,
    pub count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationOperator {
    DropRule,
    DropCondition,
    DropFact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MutationDecl {
    pub id: String,
    pub operator: MutationOperator,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relation: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub except: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub must_fail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

impl MutationDecl {
    pub(crate) fn excludes_condition(&self, rule: &str, condition: Option<&str>) -> bool {
        self.except.iter().any(|except| {
            except == rule
                || condition.is_some_and(|condition| {
                    except
                        .strip_prefix(rule)
                        .and_then(|suffix| suffix.strip_prefix('.'))
                        == Some(condition)
                })
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Artifact {
    pub name: String,
    /// Every comment written before the `spec` keyword: the question the
    /// artifact answers, in the author's words.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
    pub relations: Vec<RelationDecl>,
    pub facts: Vec<FactDecl>,
    pub rules: Vec<RuleDecl>,
    pub expectations: Vec<ExpectationDecl>,
    pub mutations: Vec<MutationDecl>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WalkFact {
    pub relation: String,
    pub args: Vec<String>,
    pub origin: String,
    pub confidence: f64,
    pub provenance: Vec<String>,
    pub why: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WalkExpectation {
    pub id: String,
    pub query: String,
    pub expected_count: usize,
    pub actual_count: usize,
    pub satisfied: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WalkReport {
    pub spec: String,
    pub status: String,
    pub asserted: usize,
    pub derived: usize,
    pub facts: Vec<WalkFact>,
    pub expectations: Vec<WalkExpectation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactError {
    kind: ArtifactErrorKind,
    message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArtifactErrorKind {
    InvalidArtifact,
    Evaluation,
}

impl ArtifactError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            kind: ArtifactErrorKind::InvalidArtifact,
            message: message.into(),
        }
    }

    pub(crate) fn evaluation(message: impl Into<String>) -> Self {
        Self {
            kind: ArtifactErrorKind::Evaluation,
            message: message.into(),
        }
    }

    pub(crate) fn is_invalid_artifact(&self) -> bool {
        self.kind == ArtifactErrorKind::InvalidArtifact
    }
}

impl fmt::Display for ArtifactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ArtifactError {}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TokenKind {
    Identifier(String),
    String(String),
    Integer(i64),
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Colon,
    Comma,
    Eof,
}

#[derive(Debug, Clone)]
struct Token {
    kind: TokenKind,
    offset: usize,
}

/// One source comment, kept so declarations can carry their author's prose.
#[derive(Debug, Clone)]
struct Comment {
    text: String,
    start: usize,
    end: usize,
}

struct Lexer<'a> {
    source: &'a str,
    bytes: &'a [u8],
    offset: usize,
    comments: Vec<Comment>,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            bytes: source.as_bytes(),
            offset: 0,
            comments: Vec::new(),
        }
    }

    fn tokenize(mut self) -> Result<(Vec<Token>, Vec<Comment>), ArtifactError> {
        let mut tokens = Vec::new();
        loop {
            self.skip_trivia()?;
            if self.offset >= self.bytes.len() {
                tokens.push(Token {
                    kind: TokenKind::Eof,
                    offset: self.offset,
                });
                return Ok((tokens, self.comments));
            }
            let offset = self.offset;
            let kind = match self.bytes[self.offset] {
                b'{' => {
                    self.offset += 1;
                    TokenKind::LeftBrace
                }
                b'}' => {
                    self.offset += 1;
                    TokenKind::RightBrace
                }
                b'[' => {
                    self.offset += 1;
                    TokenKind::LeftBracket
                }
                b']' => {
                    self.offset += 1;
                    TokenKind::RightBracket
                }
                b':' => {
                    self.offset += 1;
                    TokenKind::Colon
                }
                b',' => {
                    self.offset += 1;
                    TokenKind::Comma
                }
                b'"' => TokenKind::String(self.string()?),
                b'-' if self.peek(1).is_some_and(|byte| byte.is_ascii_digit()) => {
                    TokenKind::Integer(self.integer()?)
                }
                byte if byte.is_ascii_digit() => TokenKind::Integer(self.integer()?),
                byte if is_identifier_start(byte) => TokenKind::Identifier(self.identifier()),
                byte => {
                    return Err(self.error(
                        offset,
                        format!("unexpected character `{}`", char::from(byte)),
                    ));
                }
            };
            tokens.push(Token { kind, offset });
        }
    }

    fn skip_trivia(&mut self) -> Result<(), ArtifactError> {
        loop {
            while self
                .bytes
                .get(self.offset)
                .is_some_and(|byte| byte.is_ascii_whitespace())
            {
                self.offset += 1;
            }
            if self.bytes.get(self.offset..self.offset + 2) == Some(b"//") {
                let start = self.offset;
                self.offset += 2;
                let text_start = self.offset;
                while self
                    .bytes
                    .get(self.offset)
                    .is_some_and(|byte| *byte != b'\n')
                {
                    self.offset += 1;
                }
                self.comment(start, text_start, self.offset, self.offset);
                continue;
            }
            if self.bytes.get(self.offset) == Some(&b'#') {
                let start = self.offset;
                self.offset += 1;
                let text_start = self.offset;
                while self
                    .bytes
                    .get(self.offset)
                    .is_some_and(|byte| *byte != b'\n')
                {
                    self.offset += 1;
                }
                self.comment(start, text_start, self.offset, self.offset);
                continue;
            }
            if self.bytes.get(self.offset..self.offset + 2) == Some(b"/*") {
                let start = self.offset;
                self.offset += 2;
                let text_start = self.offset;
                while self.bytes.get(self.offset..self.offset + 2) != Some(b"*/") {
                    if self.offset >= self.bytes.len() {
                        return Err(self.error(start, "unterminated block comment"));
                    }
                    self.offset += 1;
                }
                let text_end = self.offset;
                self.offset += 2;
                self.comment(start, text_start, text_end, self.offset);
                continue;
            }
            return Ok(());
        }
    }

    fn comment(&mut self, start: usize, text_start: usize, text_end: usize, end: usize) {
        let text = self.source[text_start..text_end]
            .lines()
            .map(|line| {
                let line = line.trim();
                line.strip_prefix('*').map_or(line, str::trim_start)
            })
            .collect::<Vec<_>>()
            .join("\n");
        self.comments.push(Comment {
            text: text.trim().to_string(),
            start,
            end,
        });
    }

    fn identifier(&mut self) -> String {
        let start = self.offset;
        self.offset += 1;
        while self
            .bytes
            .get(self.offset)
            .is_some_and(|byte| is_identifier_continue(*byte))
        {
            self.offset += 1;
        }
        self.source[start..self.offset].to_string()
    }

    fn integer(&mut self) -> Result<i64, ArtifactError> {
        let start = self.offset;
        if self.bytes[self.offset] == b'-' {
            self.offset += 1;
        }
        while self
            .bytes
            .get(self.offset)
            .is_some_and(|byte| byte.is_ascii_digit())
        {
            self.offset += 1;
        }
        self.source[start..self.offset]
            .parse()
            .map_err(|_| self.error(start, "integer is outside the i64 range"))
    }

    fn string(&mut self) -> Result<String, ArtifactError> {
        let start = self.offset;
        self.offset += 1;
        let mut value = String::new();
        while let Some(byte) = self.bytes.get(self.offset).copied() {
            match byte {
                b'"' => {
                    self.offset += 1;
                    return Ok(value);
                }
                b'\\' => {
                    self.offset += 1;
                    let Some(escaped) = self.bytes.get(self.offset).copied() else {
                        return Err(self.error(start, "unterminated string"));
                    };
                    self.offset += 1;
                    value.push(match escaped {
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'"' => '"',
                        b'\\' => '\\',
                        _ => {
                            return Err(self.error(
                                self.offset - 1,
                                format!("unsupported escape `\\{}`", char::from(escaped)),
                            ));
                        }
                    });
                }
                _ if byte.is_ascii() => {
                    value.push(char::from(byte));
                    self.offset += 1;
                }
                _ => {
                    let tail = &self.source[self.offset..];
                    let character = tail.chars().next().expect("valid UTF-8 source");
                    value.push(character);
                    self.offset += character.len_utf8();
                }
            }
        }
        Err(self.error(start, "unterminated string"))
    }

    fn peek(&self, distance: usize) -> Option<u8> {
        self.bytes.get(self.offset + distance).copied()
    }

    fn error(&self, offset: usize, message: impl Into<String>) -> ArtifactError {
        let (line, column) = line_column(self.source, offset);
        ArtifactError::new(format!("{line}:{column}: {}", message.into()))
    }
}

fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_identifier_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Whitespace spanning at most one line break: the gap between a comment
/// and the declaration it documents.
fn touches(gap: &str) -> bool {
    gap.chars().all(char::is_whitespace) && gap.matches('\n').count() <= 1
}

fn line_column(source: &str, offset: usize) -> (usize, usize) {
    let prefix = &source[..offset.min(source.len())];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rsplit_once('\n')
        .map_or(prefix.chars().count() + 1, |(_, tail)| {
            tail.chars().count() + 1
        });
    (line, column)
}

#[derive(Debug, Clone)]
enum RawValue {
    Identifier(String),
    String(String),
    Integer(i64),
    List(Vec<RawValue>),
    Map(Vec<(String, RawValue)>),
}

#[derive(Debug)]
struct RawBlock {
    kind: String,
    name: String,
    fields: BTreeMap<String, RawValue>,
    doc: Option<String>,
    start: usize,
    end: usize,
}

struct ParsedSpec {
    name: String,
    doc: Option<String>,
    blocks: Vec<RawBlock>,
}

struct Parser<'a> {
    source: &'a str,
    tokens: Vec<Token>,
    comments: Vec<Comment>,
    cursor: usize,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Result<Self, ArtifactError> {
        let (tokens, comments) = Lexer::new(source).tokenize()?;
        Ok(Self {
            source,
            tokens,
            comments,
            cursor: 0,
        })
    }

    fn parse(mut self) -> Result<ParsedSpec, ArtifactError> {
        let spec_offset = self.current().offset;
        self.expect_keyword("spec")?;
        let name = self.expect_identifier("spec name")?;
        self.expect(TokenKind::LeftBrace, "`{`")?;
        let mut blocks = Vec::new();
        while !self.at(&TokenKind::RightBrace) {
            if self.at(&TokenKind::Eof) {
                return Err(self.error("expected `}` to close spec"));
            }
            blocks.push(self.block()?);
        }
        self.advance();
        self.expect(TokenKind::Eof, "end of file")?;
        let doc = self.assign_docs(spec_offset, &mut blocks);
        Ok(ParsedSpec { name, doc, blocks })
    }

    /// Attach comments to declarations. Everything before `spec` documents
    /// the artifact. A comment block touching the line above a declaration
    /// documents that declaration, as does a comment trailing its closing
    /// brace. Comments separated by a blank line are section headings and
    /// belong to nobody.
    fn assign_docs(&self, spec_offset: usize, blocks: &mut [RawBlock]) -> Option<String> {
        let mut used = vec![false; self.comments.len()];
        let preamble: Vec<usize> = (0..self.comments.len())
            .filter(|index| self.comments[*index].end <= spec_offset)
            .collect();
        preamble.iter().for_each(|index| used[*index] = true);
        let spec_doc = self.join_comments(&preamble);

        let mut trailing = vec![None; blocks.len()];
        for (block_index, block) in blocks.iter().enumerate() {
            let candidate = self.comments.iter().position(|comment| {
                comment.start > block.end && !self.source[block.end..comment.start].contains('\n')
            });
            if let Some(index) = candidate.filter(|index| !used[*index]) {
                used[index] = true;
                trailing[block_index] = Some(index);
            }
        }

        for (block_index, block) in blocks.iter_mut().enumerate() {
            let mut leading = Vec::new();
            let mut boundary = block.start;
            for index in (0..self.comments.len()).rev() {
                let comment = &self.comments[index];
                if used[index] || comment.end > boundary {
                    continue;
                }
                if !touches(&self.source[comment.end..boundary]) {
                    break;
                }
                leading.push(index);
                boundary = comment.start;
            }
            leading.reverse();
            leading.iter().for_each(|index| used[*index] = true);
            let mut parts = Vec::new();
            parts.extend(self.join_comments(&leading));
            parts.extend(trailing[block_index].map(|index| self.comments[index].text.clone()));
            let doc = parts.join("\n");
            block.doc = (!doc.trim().is_empty()).then(|| doc.trim().to_string());
        }
        spec_doc
    }

    fn join_comments(&self, indices: &[usize]) -> Option<String> {
        let mut text = String::new();
        for (position, index) in indices.iter().enumerate() {
            if position > 0 {
                let previous = &self.comments[indices[position - 1]];
                let gap = &self.source[previous.end..self.comments[*index].start];
                text.push('\n');
                if gap.matches('\n').count() > 1 {
                    text.push('\n');
                }
            }
            text.push_str(&self.comments[*index].text);
        }
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_string())
    }

    fn block(&mut self) -> Result<RawBlock, ArtifactError> {
        let start = self.current().offset;
        let kind = self.expect_identifier("block kind")?;
        let name = self.expect_identifier("block name")?;
        self.expect(TokenKind::LeftBrace, "`{`")?;
        let mut fields = BTreeMap::new();
        while !self.at(&TokenKind::RightBrace) {
            if self.at(&TokenKind::Eof) {
                return Err(self.error(format!("expected `}}` to close {kind} `{name}`")));
            }
            let field = self.expect_identifier("field name")?;
            self.expect(TokenKind::Colon, "`:`")?;
            let value = self.value()?;
            if fields.insert(field.clone(), value).is_some() {
                return Err(self.error(format!("duplicate field `{field}` in {kind} `{name}`")));
            }
            if self.at(&TokenKind::Comma) {
                self.advance();
            }
        }
        let end = self.current().offset;
        self.advance();
        Ok(RawBlock {
            kind,
            name,
            fields,
            doc: None,
            start,
            end,
        })
    }

    fn value(&mut self) -> Result<RawValue, ArtifactError> {
        match self.current().kind.clone() {
            TokenKind::Identifier(value) => {
                self.advance();
                Ok(RawValue::Identifier(value))
            }
            TokenKind::String(value) => {
                self.advance();
                Ok(RawValue::String(value))
            }
            TokenKind::Integer(value) => {
                self.advance();
                Ok(RawValue::Integer(value))
            }
            TokenKind::LeftBracket => {
                self.advance();
                let mut values = Vec::new();
                while !self.at(&TokenKind::RightBracket) {
                    if self.at(&TokenKind::LeftBracket) {
                        return Err(self.error("nested lists are not supported"));
                    }
                    values.push(self.value()?);
                    if self.at(&TokenKind::Comma) {
                        self.advance();
                        if self.at(&TokenKind::RightBracket) {
                            break;
                        }
                    } else if !self.at(&TokenKind::RightBracket) {
                        return Err(self.error("expected `,` or `]` in list"));
                    }
                }
                self.expect(TokenKind::RightBracket, "`]`")?;
                Ok(RawValue::List(values))
            }
            TokenKind::LeftBrace => {
                self.advance();
                let mut entries = Vec::new();
                let mut names = BTreeSet::new();
                while !self.at(&TokenKind::RightBrace) {
                    if self.at(&TokenKind::Eof) {
                        return Err(self.error("expected `}` to close map"));
                    }
                    let name = self.expect_identifier("map entry name")?;
                    if !names.insert(name.clone()) {
                        return Err(self.error(format!("duplicate map entry `{name}`")));
                    }
                    self.expect(TokenKind::Colon, "`:`")?;
                    entries.push((name, self.value()?));
                    if self.at(&TokenKind::Comma) {
                        self.advance();
                    }
                }
                self.advance();
                Ok(RawValue::Map(entries))
            }
            _ => Err(self.error("expected a value")),
        }
    }

    fn expect_keyword(&mut self, expected: &str) -> Result<(), ArtifactError> {
        let actual = self.expect_identifier(expected)?;
        if actual == expected {
            Ok(())
        } else {
            Err(self.error(format!("expected `{expected}`, found `{actual}`")))
        }
    }

    fn expect_identifier(&mut self, description: &str) -> Result<String, ArtifactError> {
        match self.current().kind.clone() {
            TokenKind::Identifier(value) => {
                self.advance();
                Ok(value)
            }
            _ => Err(self.error(format!("expected {description}"))),
        }
    }

    fn expect(&mut self, kind: TokenKind, label: &str) -> Result<(), ArtifactError> {
        if self.at(&kind) {
            self.advance();
            Ok(())
        } else {
            Err(self.error(format!("expected {label}")))
        }
    }

    fn at(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(&self.current().kind) == std::mem::discriminant(kind)
    }

    fn current(&self) -> &Token {
        &self.tokens[self.cursor]
    }

    fn advance(&mut self) {
        if self.cursor + 1 < self.tokens.len() {
            self.cursor += 1;
        }
    }

    fn error(&self, message: impl Into<String>) -> ArtifactError {
        let (line, column) = line_column(self.source, self.current().offset);
        ArtifactError::new(format!("{line}:{column}: {}", message.into()))
    }
}

pub fn parse_artifact(source: &str) -> Result<Artifact, ArtifactError> {
    let parsed = Parser::new(source)?.parse()?;
    Artifact::from_blocks(parsed)
}

impl Artifact {
    fn from_blocks(parsed: ParsedSpec) -> Result<Self, ArtifactError> {
        let ParsedSpec { name, doc, blocks } = parsed;
        let mut artifact = Artifact {
            name,
            doc,
            relations: Vec::new(),
            facts: Vec::new(),
            rules: Vec::new(),
            expectations: Vec::new(),
            mutations: Vec::new(),
        };
        let mut ids: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

        for mut block in blocks {
            if !ids
                .entry(block.kind.clone())
                .or_default()
                .insert(block.name.clone())
            {
                return Err(ArtifactError::new(format!(
                    "duplicate {} `{}`",
                    block.kind, block.name
                )));
            }

            match block.kind.as_str() {
                "relation" => {
                    let args = take_list(&mut block, "args")?
                        .into_iter()
                        .map(
                            |value| match raw_text(value, "relation argument type")?.as_str() {
                                "symbol" => Ok(ValueType::Symbol),
                                "integer" => Ok(ValueType::Integer),
                                other => Err(ArtifactError::new(format!(
                                    "relation `{}` has unknown argument type `{other}`",
                                    block.name
                                ))),
                            },
                        )
                        .collect::<Result<Vec<_>, _>>()?;
                    let roles = take_optional_text_list(&mut block, "roles")?;
                    let reads = take_optional_text(&mut block, "reads")?;
                    validate_roles(&block, &args, &roles)?;
                    if let Some(template) = reads.as_deref() {
                        validate_reads(&block, template, &roles, args.len())?;
                    }
                    reject_unknown_fields(&block)?;
                    artifact.relations.push(RelationDecl {
                        name: block.name,
                        args,
                        roles,
                        reads,
                        doc: block.doc,
                    });
                }
                "fact" => {
                    let relation = take_text(&mut block, "relation")?;
                    let args = take_list(&mut block, "args")?
                        .into_iter()
                        .map(raw_fact_value)
                        .collect::<Result<Vec<_>, _>>()?;
                    let confidence = match block.fields.remove("confidence") {
                        Some(RawValue::Integer(value @ 0..=100)) => value as f64 / 100.0,
                        Some(RawValue::Integer(value)) => {
                            return Err(ArtifactError::new(format!(
                                "fact `{}` confidence must be between 0 and 100, got {value}",
                                block.name
                            )));
                        }
                        Some(_) => {
                            return Err(ArtifactError::new(format!(
                                "fact `{}` confidence must be an integer percentage",
                                block.name
                            )));
                        }
                        None => 1.0,
                    };
                    let provenance = match block.fields.remove("provenance") {
                        Some(RawValue::List(values)) => values
                            .into_iter()
                            .map(|value| raw_text(value, "provenance value"))
                            .collect::<Result<Vec<_>, _>>()?,
                        Some(_) => {
                            return Err(ArtifactError::new(format!(
                                "fact `{}` provenance must be a list",
                                block.name
                            )));
                        }
                        None => Vec::new(),
                    };
                    reject_unknown_fields(&block)?;
                    artifact.facts.push(FactDecl {
                        id: block.name,
                        relation,
                        args,
                        confidence,
                        provenance,
                        doc: block.doc,
                    });
                }
                "rule" => {
                    let derive = take_text(&mut block, "derive")?;
                    let (when, condition_ids) = take_rule_conditions(&mut block)?;
                    if when.is_empty() {
                        return Err(ArtifactError::new(format!(
                            "rule `{}` must contain at least one condition",
                            block.name
                        )));
                    }
                    reject_unknown_fields(&block)?;
                    artifact.rules.push(RuleDecl {
                        id: block.name,
                        derive,
                        when,
                        condition_ids,
                        doc: block.doc,
                    });
                }
                "expect" => {
                    let query = take_text(&mut block, "query")?;
                    let count = match block.fields.remove("count") {
                        Some(RawValue::Integer(value)) if value >= 0 => value as usize,
                        Some(RawValue::Integer(value)) => {
                            return Err(ArtifactError::new(format!(
                                "expect `{}` count cannot be negative: {value}",
                                block.name
                            )));
                        }
                        Some(_) => {
                            return Err(ArtifactError::new(format!(
                                "expect `{}` count must be an integer",
                                block.name
                            )));
                        }
                        None => 1,
                    };
                    reject_unknown_fields(&block)?;
                    artifact.expectations.push(ExpectationDecl {
                        id: block.name,
                        query,
                        count,
                        doc: block.doc,
                    });
                }
                "mutation" => {
                    let operator = match take_text(&mut block, "operator")?.as_str() {
                        "drop_rule" => MutationOperator::DropRule,
                        "drop_condition" => MutationOperator::DropCondition,
                        "drop_fact" => MutationOperator::DropFact,
                        other => {
                            return Err(ArtifactError::new(format!(
                                "mutation `{}` has unknown operator `{other}`",
                                block.name
                            )));
                        }
                    };
                    let relation = take_optional_text(&mut block, "relation")?;
                    let except = take_optional_text_list(&mut block, "except")?;
                    let must_fail = take_optional_text(&mut block, "must_fail")?;
                    reject_unknown_fields(&block)?;
                    artifact.mutations.push(MutationDecl {
                        id: block.name,
                        operator,
                        relation,
                        except,
                        must_fail,
                        doc: block.doc,
                    });
                }
                other => {
                    return Err(ArtifactError::new(format!(
                        "unknown block kind `{other}`; expected relation, fact, rule, expect, or mutation"
                    )));
                }
            }
        }

        artifact
            .relations
            .sort_by(|left, right| left.name.cmp(&right.name));
        artifact.facts.sort_by(|left, right| left.id.cmp(&right.id));
        artifact.rules.sort_by(|left, right| left.id.cmp(&right.id));
        artifact
            .expectations
            .sort_by(|left, right| left.id.cmp(&right.id));
        artifact
            .mutations
            .sort_by(|left, right| left.id.cmp(&right.id));
        artifact.validate_mutations()?;
        Ok(artifact)
    }

    fn validate_mutations(&self) -> Result<(), ArtifactError> {
        let relations: BTreeSet<_> = self
            .relations
            .iter()
            .map(|relation| relation.name.as_str())
            .collect();
        let rules: BTreeSet<_> = self.rules.iter().map(|rule| rule.id.as_str()).collect();
        let facts: BTreeSet<_> = self.facts.iter().map(|fact| fact.id.as_str()).collect();
        let expectations: BTreeSet<_> = self
            .expectations
            .iter()
            .map(|expectation| expectation.id.as_str())
            .collect();
        let mut policy_signatures = BTreeMap::new();

        for mutation in &self.mutations {
            match mutation.operator {
                MutationOperator::DropRule | MutationOperator::DropCondition => {
                    if mutation.relation.is_some() {
                        return Err(ArtifactError::new(format!(
                            "mutation `{}` operator `{}` does not accept `relation`",
                            mutation.id,
                            mutation.operator.as_str()
                        )));
                    }
                    if self.rules.is_empty() {
                        return Err(ArtifactError::new(format!(
                            "mutation `{}` has no rules to target",
                            mutation.id
                        )));
                    }
                    if mutation.operator == MutationOperator::DropCondition {
                        validate_condition_exceptions(mutation, &rules, &self.rules)?;
                    } else {
                        validate_exceptions(mutation, &rules, "rule")?;
                    }
                }
                MutationOperator::DropFact => {
                    let relation = mutation.relation.as_deref().ok_or_else(|| {
                        ArtifactError::new(format!(
                            "mutation `{}` operator `drop_fact` requires `relation`",
                            mutation.id
                        ))
                    })?;
                    if !relations.contains(relation) {
                        return Err(ArtifactError::new(format!(
                            "mutation `{}` references unknown relation `{relation}`",
                            mutation.id
                        )));
                    }
                    let matching_facts: BTreeSet<_> = self
                        .facts
                        .iter()
                        .filter(|fact| fact.relation == relation)
                        .map(|fact| fact.id.as_str())
                        .collect();
                    if matching_facts.is_empty() {
                        return Err(ArtifactError::new(format!(
                            "mutation `{}` has no facts of relation `{relation}` to target",
                            mutation.id
                        )));
                    }
                    validate_exceptions(mutation, &facts, "fact")?;
                    if let Some(except) = mutation
                        .except
                        .iter()
                        .find(|except| !matching_facts.contains(except.as_str()))
                    {
                        return Err(ArtifactError::new(format!(
                            "mutation `{}` excludes fact `{except}`, which is not of relation `{relation}`",
                            mutation.id
                        )));
                    }
                }
            }

            if let Some(expectation) = mutation.must_fail.as_deref() {
                if !expectations.contains(expectation) {
                    return Err(ArtifactError::new(format!(
                        "mutation `{}` requires unknown expectation `{expectation}` to fail",
                        mutation.id
                    )));
                }
            }

            let signature = (
                mutation.operator.as_str(),
                mutation.relation.as_deref(),
                self.effective_mutation_targets(mutation),
                mutation.must_fail.as_deref(),
            );
            if let Some(existing) = policy_signatures.insert(signature, mutation.id.as_str()) {
                return Err(ArtifactError::new(format!(
                    "mutations `{existing}` and `{}` declare identical policies",
                    mutation.id
                )));
            }
        }
        Ok(())
    }

    fn effective_mutation_targets(&self, mutation: &MutationDecl) -> Vec<String> {
        match mutation.operator {
            MutationOperator::DropRule => self
                .rules
                .iter()
                .filter(|rule| !mutation.except.contains(&rule.id))
                .map(|rule| rule.id.clone())
                .collect(),
            MutationOperator::DropCondition => self
                .rules
                .iter()
                .flat_map(|rule| {
                    rule.when.iter().enumerate().filter_map(move |(index, _)| {
                        let condition = rule.condition_id(index);
                        if mutation.excludes_condition(&rule.id, condition) {
                            return None;
                        }
                        let condition =
                            condition.map_or_else(|| format!("#{}", index + 1), str::to_string);
                        Some(format!("{}.{condition}", rule.id))
                    })
                })
                .collect(),
            MutationOperator::DropFact => self
                .facts
                .iter()
                .filter(|fact| {
                    mutation.relation.as_deref() == Some(fact.relation.as_str())
                        && !mutation.except.contains(&fact.id)
                })
                .map(|fact| fact.id.clone())
                .collect(),
        }
    }
}

impl MutationOperator {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DropRule => "drop_rule",
            Self::DropCondition => "drop_condition",
            Self::DropFact => "drop_fact",
        }
    }
}

fn validate_exceptions(
    mutation: &MutationDecl,
    targets: &BTreeSet<&str>,
    target_kind: &str,
) -> Result<(), ArtifactError> {
    validate_unique_exceptions(mutation)?;
    for except in &mutation.except {
        if !targets.contains(except.as_str()) {
            return Err(ArtifactError::new(format!(
                "mutation `{}` excludes unknown {target_kind} `{except}`",
                mutation.id
            )));
        }
    }
    Ok(())
}

fn validate_condition_exceptions(
    mutation: &MutationDecl,
    rules: &BTreeSet<&str>,
    rule_declarations: &[RuleDecl],
) -> Result<(), ArtifactError> {
    validate_unique_exceptions(mutation)?;
    for except in &mutation.except {
        if !rules.contains(except.as_str()) && !named_condition_exists(rule_declarations, except) {
            return Err(ArtifactError::new(format!(
                "mutation `{}` excludes unknown rule or named condition `{except}`",
                mutation.id
            )));
        }
    }
    Ok(())
}

fn validate_unique_exceptions(mutation: &MutationDecl) -> Result<(), ArtifactError> {
    let mut seen = BTreeSet::new();
    for except in &mutation.except {
        if !seen.insert(except) {
            return Err(ArtifactError::new(format!(
                "mutation `{}` excludes `{except}` more than once",
                mutation.id
            )));
        }
    }
    Ok(())
}

fn named_condition_exists(rules: &[RuleDecl], reference: &str) -> bool {
    let Some((rule_id, condition_id)) = reference.split_once('.') else {
        return false;
    };
    rules.iter().any(|rule| {
        rule.id == rule_id
            && rule
                .condition_ids
                .iter()
                .any(|candidate| candidate == condition_id)
    })
}

fn validate_roles(
    block: &RawBlock,
    args: &[ValueType],
    roles: &[String],
) -> Result<(), ArtifactError> {
    if roles.is_empty() {
        return Ok(());
    }
    if roles.len() != args.len() {
        return Err(ArtifactError::new(format!(
            "relation `{}` declares {} roles for {} arguments",
            block.name,
            roles.len(),
            args.len()
        )));
    }
    let mut seen = BTreeSet::new();
    for role in roles {
        if role.is_empty() || !role.bytes().all(is_identifier_continue) {
            return Err(ArtifactError::new(format!(
                "relation `{}` role `{role}` must be an identifier",
                block.name
            )));
        }
        if !seen.insert(role.as_str()) {
            return Err(ArtifactError::new(format!(
                "relation `{}` repeats role `{role}`",
                block.name
            )));
        }
    }
    Ok(())
}

/// Every `{placeholder}` in a `reads` template must name a role or an
/// argument position, so a fact can always be read back as a sentence.
fn validate_reads(
    block: &RawBlock,
    template: &str,
    roles: &[String],
    arity: usize,
) -> Result<(), ArtifactError> {
    for placeholder in template_placeholders(template).map_err(|message| {
        ArtifactError::new(format!("relation `{}` reads {message}", block.name))
    })? {
        let known = roles.iter().any(|role| role == placeholder)
            || placeholder
                .parse::<usize>()
                .is_ok_and(|index| index < arity);
        if !known {
            return Err(ArtifactError::new(format!(
                "relation `{}` reads placeholder `{{{placeholder}}}` names no role or argument position",
                block.name
            )));
        }
    }
    Ok(())
}

pub(crate) fn template_placeholders(template: &str) -> Result<Vec<&str>, String> {
    let mut placeholders = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            return Err("template has an unclosed `{`".to_string());
        };
        placeholders.push(&after[..close]);
        rest = &after[close + 1..];
    }
    Ok(placeholders)
}

fn take_list(block: &mut RawBlock, field: &str) -> Result<Vec<RawValue>, ArtifactError> {
    match block.fields.remove(field) {
        Some(RawValue::List(values)) => Ok(values),
        Some(_) => Err(ArtifactError::new(format!(
            "{} `{}` field `{field}` must be a list",
            block.kind, block.name
        ))),
        None => Err(ArtifactError::new(format!(
            "{} `{}` is missing required field `{field}`",
            block.kind, block.name
        ))),
    }
}

fn take_text(block: &mut RawBlock, field: &str) -> Result<String, ArtifactError> {
    let value = block.fields.remove(field).ok_or_else(|| {
        ArtifactError::new(format!(
            "{} `{}` is missing required field `{field}`",
            block.kind, block.name
        ))
    })?;
    raw_text(value, field)
}

fn take_optional_text(block: &mut RawBlock, field: &str) -> Result<Option<String>, ArtifactError> {
    block
        .fields
        .remove(field)
        .map(|value| raw_text(value, field))
        .transpose()
}

fn take_optional_text_list(
    block: &mut RawBlock,
    field: &str,
) -> Result<Vec<String>, ArtifactError> {
    match block.fields.remove(field) {
        Some(RawValue::List(values)) => values
            .into_iter()
            .map(|value| raw_text(value, field))
            .collect(),
        Some(_) => Err(ArtifactError::new(format!(
            "{} `{}` field `{field}` must be a list",
            block.kind, block.name
        ))),
        None => Ok(Vec::new()),
    }
}

fn take_rule_conditions(block: &mut RawBlock) -> Result<(Vec<String>, Vec<String>), ArtifactError> {
    let value = block.fields.remove("when").ok_or_else(|| {
        ArtifactError::new(format!(
            "rule `{}` is missing required field `when`",
            block.name
        ))
    })?;
    match value {
        RawValue::List(values) => Ok((
            values
                .into_iter()
                .map(|value| raw_text(value, "rule condition"))
                .collect::<Result<Vec<_>, _>>()?,
            Vec::new(),
        )),
        RawValue::Map(entries) => {
            let mut ids = Vec::with_capacity(entries.len());
            let mut conditions = Vec::with_capacity(entries.len());
            for (id, value) in entries {
                ids.push(id);
                conditions.push(raw_text(value, "named rule condition")?);
            }
            Ok((conditions, ids))
        }
        _ => Err(ArtifactError::new(format!(
            "rule `{}` field `when` must be a list or named map",
            block.name
        ))),
    }
}

fn raw_text(value: RawValue, description: &str) -> Result<String, ArtifactError> {
    match value {
        RawValue::Identifier(value) | RawValue::String(value) => Ok(value),
        _ => Err(ArtifactError::new(format!(
            "{description} must be an identifier or string"
        ))),
    }
}

fn raw_fact_value(value: RawValue) -> Result<FactValue, ArtifactError> {
    match value {
        RawValue::Identifier(value) | RawValue::String(value) => Ok(FactValue::Symbol(value)),
        RawValue::Integer(value) => Ok(FactValue::Integer(value)),
        RawValue::List(_) => Err(ArtifactError::new(
            "fact arguments cannot contain nested lists",
        )),
        RawValue::Map(_) => Err(ArtifactError::new("fact arguments cannot contain maps")),
    }
}

fn reject_unknown_fields(block: &RawBlock) -> Result<(), ArtifactError> {
    if let Some(field) = block.fields.keys().next() {
        Err(ArtifactError::new(format!(
            "{} `{}` has unknown field `{field}`",
            block.kind, block.name
        )))
    } else {
        Ok(())
    }
}

pub(crate) struct ArtifactEvaluation {
    pub(crate) artifact: Artifact,
    pub(crate) engine: Engine,
    pub(crate) report: WalkReport,
}

pub(crate) fn evaluate_artifact(source: &str) -> Result<ArtifactEvaluation, ArtifactError> {
    let artifact = parse_artifact(source)?;
    evaluate_parsed_artifact(artifact)
}

pub(crate) fn evaluate_parsed_artifact(
    artifact: Artifact,
) -> Result<ArtifactEvaluation, ArtifactError> {
    let schemas: BTreeMap<String, Vec<ValueType>> = artifact
        .relations
        .iter()
        .map(|relation| (relation.name.clone(), relation.args.clone()))
        .collect();

    validate_facts(&artifact.facts, &schemas)?;
    if let Some(rule) = artifact.rules.iter().find(|rule| rule.when.is_empty()) {
        return Err(ArtifactError::new(format!(
            "rule `{}` must contain at least one condition",
            rule.id
        )));
    }
    let compiled_rules = artifact
        .rules
        .iter()
        .map(|rule| compile_rule(rule, &schemas))
        .collect::<Result<Vec<_>, _>>()?;
    let derived_relations: BTreeMap<_, _> = compiled_rules
        .iter()
        .map(|rule| (rule.head_relation.as_str(), rule.id.as_str()))
        .collect();
    for fact in &artifact.facts {
        if let Some(rule_id) = derived_relations.get(fact.relation.as_str()) {
            return Err(ArtifactError::new(format!(
                "fact `{}` asserts relation `{}`, which is derived by rule `{rule_id}`",
                fact.id, fact.relation
            )));
        }
    }
    for expectation in &artifact.expectations {
        validate_query(expectation, &schemas)?;
    }

    let mut engine = Engine::new();
    for rule in compiled_rules {
        engine
            .install_program(&rule.source)
            .map_err(|error| ArtifactError::new(format!("install rule: {error}")))?;
    }

    for fact in &artifact.facts {
        let args = intern_fact_args(&mut engine, &fact.args);
        let mut provenance = fact.provenance.clone();
        provenance.push(fact.id.clone());
        engine.declare(
            &fact.relation,
            &args,
            Ann::base(fact.confidence, provenance),
        );
    }
    engine.run();

    let mut facts = Vec::new();
    for relation in &artifact.relations {
        let Some((_, stored)) = engine
            .relations_iter()
            .find(|(name, _)| name.as_str() == relation.name)
        else {
            continue;
        };
        for row in &stored.rows {
            let origin = if row
                .fact
                .supports
                .iter()
                .any(|support| matches!(support, Support::Base))
            {
                "asserted"
            } else {
                "derived"
            };
            facts.push(WalkFact {
                relation: relation.name.clone(),
                args: row
                    .key
                    .iter()
                    .map(|value| engine.interner.display(value))
                    .collect(),
                origin: origin.to_string(),
                confidence: row.fact.ann.conf,
                provenance: row.fact.ann.prov.iter().cloned().collect(),
                why: engine.why(&relation.name, &row.key),
            });
        }
    }
    facts.sort_by(|left, right| {
        (&left.relation, &left.args, &left.origin).cmp(&(
            &right.relation,
            &right.args,
            &right.origin,
        ))
    });

    let mut expectations = Vec::new();
    for expectation in &artifact.expectations {
        let actual_count = engine
            .ask(&expectation.query)
            .map_err(|error| {
                ArtifactError::evaluation(format!("evaluate expect `{}`: {error}", expectation.id))
            })?
            .len();
        expectations.push(WalkExpectation {
            id: expectation.id.clone(),
            query: expectation.query.clone(),
            expected_count: expectation.count,
            actual_count,
            satisfied: actual_count == expectation.count,
        });
    }

    let asserted_count = facts
        .iter()
        .filter(|fact| fact.origin == "asserted")
        .count();
    let derived_count = facts.len() - asserted_count;
    let status = if expectations.iter().all(|expectation| expectation.satisfied) {
        "clean"
    } else {
        "incomplete"
    };

    let report = WalkReport {
        spec: artifact.name.clone(),
        status: status.to_string(),
        asserted: asserted_count,
        derived: derived_count,
        facts,
        expectations,
    };

    Ok(ArtifactEvaluation {
        artifact,
        engine,
        report,
    })
}

pub fn walk_artifact(source: &str) -> Result<WalkReport, ArtifactError> {
    Ok(evaluate_artifact(source)?.report)
}

fn validate_facts(
    facts: &[FactDecl],
    schemas: &BTreeMap<String, Vec<ValueType>>,
) -> Result<(), ArtifactError> {
    for fact in facts {
        let schema = schemas.get(&fact.relation).ok_or_else(|| {
            ArtifactError::new(format!(
                "fact `{}` references unknown relation `{}`",
                fact.id, fact.relation
            ))
        })?;
        if schema.len() != fact.args.len() {
            return Err(ArtifactError::new(format!(
                "relation `{}` expects {} arguments, got {} in fact `{}`",
                fact.relation,
                schema.len(),
                fact.args.len(),
                fact.id
            )));
        }
        for (position, (expected, actual)) in schema.iter().zip(&fact.args).enumerate() {
            let matches = matches!(
                (expected, actual),
                (ValueType::Symbol, FactValue::Symbol(_))
                    | (ValueType::Integer, FactValue::Integer(_))
            );
            if !matches {
                return Err(ArtifactError::new(format!(
                    "fact `{}` argument {} for relation `{}` must be {}",
                    fact.id,
                    position + 1,
                    fact.relation,
                    value_type_name(expected)
                )));
            }
        }
    }
    Ok(())
}

struct CompiledRule {
    id: String,
    head_relation: String,
    source: String,
}

fn compile_rule(
    rule: &RuleDecl,
    schemas: &BTreeMap<String, Vec<ValueType>>,
) -> Result<CompiledRule, ArtifactError> {
    let source = format!("{}: {} :- {}.", rule.id, rule.derive, rule.when.join(", "));
    let clauses = parse_program(&source)
        .map_err(|error| ArtifactError::new(format!("rule `{}`: {error}", rule.id)))?;
    if clauses.len() != 1 || clauses[0].is_fact {
        return Err(ArtifactError::new(format!(
            "rule `{}` must compile to exactly one rule",
            rule.id
        )));
    }
    validate_clause(&rule.id, &clauses[0], schemas)?;
    Ok(CompiledRule {
        id: rule.id.clone(),
        head_relation: clauses[0].head.pred.clone(),
        source,
    })
}

fn validate_query(
    expectation: &ExpectationDecl,
    schemas: &BTreeMap<String, Vec<ValueType>>,
) -> Result<(), ArtifactError> {
    let clauses = parse_program(&format!("{}.", expectation.query))
        .map_err(|error| ArtifactError::new(format!("expect `{}`: {error}", expectation.id)))?;
    if clauses.len() != 1 || !clauses[0].is_fact {
        return Err(ArtifactError::new(format!(
            "expect `{}` query must be one atom",
            expectation.id
        )));
    }
    let mut variables = BTreeMap::new();
    validate_atom(
        &format!("expect `{}`", expectation.id),
        &clauses[0].head,
        schemas,
        &mut variables,
    )
}

fn validate_clause(
    rule_id: &str,
    clause: &Clause,
    schemas: &BTreeMap<String, Vec<ValueType>>,
) -> Result<(), ArtifactError> {
    let owner = format!("rule `{rule_id}`");
    let mut variables = BTreeMap::new();
    for literal in &clause.body {
        match literal {
            Lit::Pos(atom) | Lit::Neg(atom) => {
                validate_atom(&owner, atom, schemas, &mut variables)?;
            }
            Lit::Now(_) => {
                return Err(ArtifactError::new(format!(
                    "{owner} uses `now/1`, which is unavailable without a deterministic artifact clock"
                )));
            }
            Lit::Cmp(..) => {}
        }
    }
    validate_atom(&owner, &clause.head, schemas, &mut variables)?;
    for literal in &clause.body {
        if let Lit::Cmp(operator, left, right) = literal {
            validate_comparison(&owner, operator, left, right, &mut variables)?;
        }
    }
    Ok(())
}

fn validate_comparison(
    owner: &str,
    operator: &CmpOp,
    left: &Term,
    right: &Expr,
    variables: &mut BTreeMap<String, ValueType>,
) -> Result<(), ArtifactError> {
    match operator {
        CmpOp::Lt | CmpOp::Le | CmpOp::Gt | CmpOp::Ge => {
            let message = "ordering comparison requires integer operands";
            validate_term_type(owner, left, &ValueType::Integer, variables)
                .map_err(|_| ArtifactError::new(format!("{owner} {message}")))?;
            validate_integer_expression(owner, right, variables, message)
        }
        CmpOp::Eq | CmpOp::Ne => match right {
            Expr::T(right) => validate_equality_terms(owner, left, right, variables),
            Expr::Add(..) | Expr::Sub(..) => {
                let message = "arithmetic expression requires integer operands";
                validate_term_type(owner, left, &ValueType::Integer, variables)
                    .map_err(|_| ArtifactError::new(format!("{owner} {message}")))?;
                validate_integer_expression(owner, right, variables, message)
            }
        },
    }
}

fn validate_integer_expression(
    owner: &str,
    expression: &Expr,
    variables: &mut BTreeMap<String, ValueType>,
    message: &str,
) -> Result<(), ArtifactError> {
    match expression {
        Expr::T(term) => validate_term_type(owner, term, &ValueType::Integer, variables)
            .map_err(|_| ArtifactError::new(format!("{owner} {message}"))),
        Expr::Add(left, right) | Expr::Sub(left, right) => {
            validate_integer_expression(owner, left, variables, message)?;
            validate_integer_expression(owner, right, variables, message)
        }
    }
}

fn validate_equality_terms(
    owner: &str,
    left: &Term,
    right: &Term,
    variables: &mut BTreeMap<String, ValueType>,
) -> Result<(), ArtifactError> {
    match (
        known_term_type(left, variables),
        known_term_type(right, variables),
    ) {
        (Some(left_type), Some(right_type)) if left_type != right_type => {
            Err(ArtifactError::new(format!(
                "{owner} equality comparison has incompatible {} and {} operands",
                value_type_name(&left_type),
                value_type_name(&right_type)
            )))
        }
        (Some(expected), None) => validate_term_type(owner, right, &expected, variables),
        (None, Some(expected)) => validate_term_type(owner, left, &expected, variables),
        _ => Ok(()),
    }
}

fn known_term_type(term: &Term, variables: &BTreeMap<String, ValueType>) -> Option<ValueType> {
    match term {
        Term::Var(variable) => variables.get(variable).cloned(),
        Term::Sym(_) => Some(ValueType::Symbol),
        Term::Int(_) | Term::Agg(..) => Some(ValueType::Integer),
        Term::Wildcard => None,
    }
}

fn validate_atom(
    owner: &str,
    atom: &Atom,
    schemas: &BTreeMap<String, Vec<ValueType>>,
    variables: &mut BTreeMap<String, ValueType>,
) -> Result<(), ArtifactError> {
    let schema = schemas.get(&atom.pred).ok_or_else(|| {
        ArtifactError::new(format!(
            "{owner} references unknown relation `{}`",
            atom.pred
        ))
    })?;
    if schema.len() != atom.args.len() {
        return Err(ArtifactError::new(format!(
            "relation `{}` expects {} arguments, got {} in {owner}",
            atom.pred,
            schema.len(),
            atom.args.len()
        )));
    }
    for (term, expected) in atom.args.iter().zip(schema) {
        validate_term_type(owner, term, expected, variables)?;
    }
    Ok(())
}

fn validate_term_type(
    owner: &str,
    term: &Term,
    expected: &ValueType,
    variables: &mut BTreeMap<String, ValueType>,
) -> Result<(), ArtifactError> {
    match term {
        Term::Var(variable) => match variables.get(variable) {
            Some(actual) if actual != expected => Err(ArtifactError::new(format!(
                "{owner} variable `{variable}` has incompatible types {} and {}",
                value_type_name(actual),
                value_type_name(expected)
            ))),
            Some(_) => Ok(()),
            None => {
                variables.insert(variable.clone(), expected.clone());
                Ok(())
            }
        },
        Term::Sym(_) if *expected == ValueType::Symbol => Ok(()),
        Term::Int(_) if *expected == ValueType::Integer => Ok(()),
        Term::Wildcard => Ok(()),
        Term::Agg(function, inner) if *expected == ValueType::Integer => match function {
            crate::intern::AggFn::Count => Ok(()),
            crate::intern::AggFn::Min | crate::intern::AggFn::Max => {
                validate_term_type(owner, inner, &ValueType::Integer, variables).map_err(|_| {
                    ArtifactError::new(format!(
                        "{owner} aggregate `{}` requires integer input",
                        function.name()
                    ))
                })
            }
            crate::intern::AggFn::Sum => Err(ArtifactError::new(format!(
                "{owner} aggregate `sum` is unavailable until checked integer evaluation is implemented"
            ))),
        },
        _ => Err(ArtifactError::new(format!(
            "{owner} uses a {} value where {} is required",
            term_type_name(term),
            value_type_name(expected)
        ))),
    }
}

fn value_type_name(value_type: &ValueType) -> &'static str {
    match value_type {
        ValueType::Symbol => "symbol",
        ValueType::Integer => "integer",
    }
}

fn term_type_name(term: &Term) -> &'static str {
    match term {
        Term::Sym(_) => "symbol",
        Term::Int(_) | Term::Agg(_, _) => "integer",
        Term::Var(_) | Term::Wildcard => "unknown",
    }
}

fn intern_fact_args(engine: &mut Engine, args: &[FactValue]) -> Vec<Value> {
    args.iter()
        .map(|argument| match argument {
            FactValue::Symbol(value) => engine.sym(value),
            FactValue::Integer(value) => Value::Int(*value),
        })
        .collect()
}
