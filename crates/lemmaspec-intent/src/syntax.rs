use std::{collections::BTreeMap, fmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    pub path: String,
    /// The name in `catalog NAME { ... }`; `None` for files of top-level blocks.
    pub catalog: Option<String>,
    pub imports: Vec<Import>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    pub path: String,
    pub alias: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub kind: String,
    pub name: Option<String>,
    pub fields: Vec<Field>,
    pub line: usize,
}

impl Block {
    /// The block's fields by name. An unknown field fails as
    /// `path:line: <subject> has no field <name>`, a repeated one as
    /// `path:line: duplicate field <name> in <duplicate_in>`.
    pub(crate) fn fields_by_name(
        &self,
        path: &str,
        allowed: &[&str],
        subject: &str,
        duplicate_in: &str,
    ) -> Result<Fields<'_>, String> {
        let mut by_name = BTreeMap::new();
        for field in &self.fields {
            let at = format!("{path}:{}", field.line);
            if !allowed.contains(&field.name.as_str()) {
                return Err(format!("{at}: {subject} has no field {}", field.name));
            }
            if by_name.insert(field.name.as_str(), field).is_some() {
                return Err(format!(
                    "{at}: duplicate field {} in {duplicate_in}",
                    field.name
                ));
            }
        }
        Ok(Fields {
            by_name,
            at: format!("{path}:{}", self.line),
            subject: subject.into(),
        })
    }
}

pub(crate) struct Fields<'a> {
    by_name: BTreeMap<&'a str, &'a Field>,
    at: String,
    subject: String,
}

impl<'a> Fields<'a> {
    pub(crate) fn get(&self, name: &str) -> Option<&'a Field> {
        self.by_name.get(name).copied()
    }

    /// Fails as `path:line: <subject> needs <name>`, at the block's line.
    pub(crate) fn required(&self, name: &str) -> Result<&'a Field, String> {
        self.get(name)
            .ok_or_else(|| format!("{}: {} needs {name}", self.at, self.subject))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub value: Value,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Ident(String),
    /// `alias.id`
    Qualified(String, String),
    Str(String),
    Int(u64),
    List(Vec<Value>),
    Entry(String, Box<Value>),
}

impl Value {
    pub(crate) fn as_ident(&self) -> Option<&str> {
        match self {
            Value::Ident(word) => Some(word),
            _ => None,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Ident(id) => write!(f, "{id}"),
            Value::Qualified(alias, id) => write!(f, "{alias}.{id}"),
            Value::Str(text) => write!(f, "{text:?}"),
            Value::Int(number) => write!(f, "{number}"),
            Value::List(items) => {
                write!(f, "[")?;
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, "]")
            }
            Value::Entry(key, value) => write!(f, "{key}: {value}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Word(String),
    Str(String),
    Int(u64),
    Punct(char),
}

/// Parse one `.intent` file. `path` names the file in errors as `path:line: message`.
pub fn parse(source: &str, path: &str) -> Result<File, String> {
    let mut parser = Parser {
        tokens: lex(source, path)?,
        position: 0,
        path,
    };
    let mut file = File {
        path: path.into(),
        catalog: None,
        imports: Vec::new(),
        blocks: Vec::new(),
    };
    if parser.at_word("catalog") {
        parser.advance();
        file.catalog = Some(parser.name("catalog name")?);
        parser.expect('{')?;
        while parser.at_word("use") {
            let line = parser.line();
            parser.advance();
            let Some(Token::Str(path)) = parser.peek().cloned() else {
                return Err(parser.unexpected("import path string"));
            };
            parser.advance();
            if !parser.at_word("as") {
                return Err(parser.unexpected("as"));
            }
            parser.advance();
            let alias = parser.name("import alias")?;
            file.imports.push(Import { path, alias, line });
        }
        while !parser.at('}') {
            if parser.at_word("use") {
                return Err(parser.error("use must come before statements"));
            }
            file.blocks.push(parser.block()?);
        }
        parser.expect('}')?;
        if parser.peek().is_some() {
            return Err(parser.unexpected("end of file"));
        }
    } else {
        while parser.peek().is_some() {
            file.blocks.push(parser.block()?);
        }
    }
    Ok(file)
}

struct Parser<'a> {
    tokens: Vec<(Token, usize)>,
    position: usize,
    path: &'a str,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position).map(|(token, _)| token)
    }

    fn advance(&mut self) {
        self.position += 1;
    }

    fn line(&self) -> usize {
        self.tokens
            .get(self.position)
            .or(self.tokens.last())
            .map_or(1, |(_, line)| *line)
    }

    fn at(&self, punct: char) -> bool {
        self.peek() == Some(&Token::Punct(punct))
    }

    fn at_word(&self, word: &str) -> bool {
        matches!(self.peek(), Some(Token::Word(found)) if found == word)
    }

    fn error(&self, message: impl fmt::Display) -> String {
        format!("{}:{}: {message}", self.path, self.line())
    }

    fn unexpected(&self, expected: &str) -> String {
        let found = match self.peek() {
            None => "end of file".into(),
            Some(Token::Word(word)) => word.clone(),
            Some(Token::Str(_)) => "a string".into(),
            Some(Token::Int(number)) => number.to_string(),
            Some(Token::Punct(punct)) => format!("'{punct}'"),
        };
        self.error(format!("expected {expected}, found {found}"))
    }

    fn expect(&mut self, punct: char) -> Result<(), String> {
        if !self.at(punct) {
            return Err(self.unexpected(&format!("'{punct}'")));
        }
        self.advance();
        Ok(())
    }

    fn name(&mut self, what: &str) -> Result<String, String> {
        match self.peek() {
            Some(Token::Word(word)) if !word.contains('.') => {
                let word = word.clone();
                self.advance();
                Ok(word)
            }
            _ => Err(self.unexpected(what)),
        }
    }

    fn block(&mut self) -> Result<Block, String> {
        let line = self.line();
        let kind = self.name("block kind")?;
        let name = if self.at('{') {
            None
        } else {
            Some(self.name("block name or '{'")?)
        };
        self.expect('{')?;
        let mut fields = Vec::new();
        while !self.at('}') {
            let line = self.line();
            let name = self.name("field name")?;
            self.expect(':')?;
            let value = self.value()?;
            fields.push(Field { name, value, line });
        }
        self.expect('}')?;
        Ok(Block {
            kind,
            name,
            fields,
            line,
        })
    }

    fn value(&mut self) -> Result<Value, String> {
        if self.at('[') {
            self.advance();
            let mut items = Vec::new();
            while !self.at(']') {
                let item = self.value()?;
                if self.at(':') {
                    let Value::Ident(key) = item else {
                        return Err(self.error("a list entry key must be an identifier"));
                    };
                    self.advance();
                    items.push(Value::Entry(key, Box::new(self.value()?)));
                } else {
                    items.push(item);
                }
                if !self.at(',') {
                    break;
                }
                self.advance();
            }
            self.expect(']')?;
            return Ok(Value::List(items));
        }
        let value = match self.peek() {
            Some(Token::Str(text)) => Value::Str(text.clone()),
            Some(Token::Int(number)) => Value::Int(*number),
            Some(Token::Word(word)) => match word.split_once('.') {
                Some((alias, id)) => Value::Qualified(alias.into(), id.into()),
                None => Value::Ident(word.clone()),
            },
            _ => return Err(self.unexpected("a value")),
        };
        self.advance();
        Ok(value)
    }
}

fn lex(source: &str, path: &str) -> Result<Vec<(Token, usize)>, String> {
    let mut tokens = Vec::new();
    let mut chars = source.chars().peekable();
    let mut line = 1;
    let error = |line: usize, message: String| format!("{path}:{line}: {message}");
    while let Some(c) = chars.next() {
        match c {
            '\n' => line += 1,
            c if c.is_whitespace() => {}
            '/' if chars.peek() == Some(&'/') => while chars.next_if(|c| *c != '\n').is_some() {},
            '{' | '}' | '[' | ']' | ':' | ',' => tokens.push((Token::Punct(c), line)),
            '"' => {
                let mut text = String::new();
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => text.push(match chars.next() {
                            Some('n') => '\n',
                            Some('r') => '\r',
                            Some('t') => '\t',
                            Some(c @ ('"' | '\\')) => c,
                            Some(c) if c != '\n' => {
                                return Err(error(line, format!("unsupported escape \\{c}")))
                            }
                            _ => return Err(error(line, "unterminated string".into())),
                        }),
                        Some('\n') | None => return Err(error(line, "unterminated string".into())),
                        Some(c) => text.push(c),
                    }
                }
                tokens.push((Token::Str(text), line));
            }
            c if c.is_ascii_alphanumeric() || c == '_' => {
                let mut word = String::from(c);
                while let Some(c) =
                    chars.next_if(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '.')
                {
                    word.push(c);
                }
                tokens.push((word_token(word).map_err(|e| error(line, e))?, line));
            }
            c => return Err(error(line, format!("unexpected character {c:?}"))),
        }
    }
    Ok(tokens)
}

pub(crate) fn quote(text: &str) -> String {
    let mut quoted = String::from('"');
    for c in text.chars() {
        match c {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}

fn word_token(word: String) -> Result<Token, String> {
    if word.starts_with(|c: char| c.is_ascii_digit()) {
        return match word.parse() {
            Ok(number) if word.bytes().all(|b| b.is_ascii_digit()) => Ok(Token::Int(number)),
            _ => Err(format!("invalid integer {word}")),
        };
    }
    let parts: Vec<_> = word.split('.').collect();
    if parts.len() > 2
        || parts
            .iter()
            .any(|part| !part.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_'))
    {
        return Err(format!("invalid identifier {word}"));
    }
    Ok(Token::Word(word))
}
