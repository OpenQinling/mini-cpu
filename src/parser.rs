use std::sync::Arc;

use terl::{MakeError, Parser, Span, WithSpan};

fn skip_whitespace(p: &mut Parser<char>) {
    while p
        .next_if(|c| c.is_ascii_whitespace() && *c != '\n')
        .is_some()
    {}
}

fn parse_char(p: &mut Parser<char>, ch: char) -> terl::Result<(), terl::ParseError> {
    if p.next_if(|c| *c == ch).is_none() {
        p.unmatch(format!("expect `{}`", ch))?;
    }
    Ok(())
}

#[derive(Debug)]
struct Comment;

impl Comment {
    fn parse(p: &mut Parser<char>) -> terl::Result<bool, terl::ParseError> {
        parse_char(p, ';')?;
        while let Some(next) = p.next() {
            if *next == '\n' {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn parse_eol(p: &mut Parser<char>) -> terl::Result<bool, terl::ParseError> {
    skip_whitespace(p);
    if let Ok(term) = Comment::parse(p) {
        return Ok(term);
    }

    match p.next() {
        Some(&'\n') => Ok(false),
        None => Ok(true),
        _ => p.unmatch("expect EOL"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ident {
    literal: Arc<str>,
    buf_name: Arc<str>,
    location: Span,
}

impl Ident {
    fn parse(p: &mut Parser<char>) -> terl::Result<Self, terl::ParseError> {
        skip_whitespace(p);
        p.start_taking();
        let mut ident = String::new();
        while let Some(c) = p.next_if(|c| !c.is_whitespace() && *c != '=' && *c != ';') {
            ident.push(*c);
        }

        if ident.is_empty() {
            p.unmatch("no more ident")?;
        }

        Ok(Ident {
            literal: ident.into_boxed_str().into(),
            buf_name: p.buffer().buf_name().clone(),
            location: p.get_span(),
        })
    }
}

impl Ident {
    pub fn literal(&self) -> &Arc<str> {
        &self.literal
    }

    pub fn path(&self) -> &Arc<str> {
        &self.buf_name
    }
}

impl terl::WithSpan for Ident {
    fn get_span(&self) -> Span {
        self.location
    }
}

impl terl::WithBufName for Ident {
    fn buf_name(&self) -> &Arc<str> {
        &self.buf_name
    }
}

impl core::fmt::Display for Ident {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.literal())
    }
}

#[derive(Debug)]
pub struct Define {
    pub name: Ident,
    pub value: Ident,
}

impl Define {
    fn parse(p: &mut Parser<char>) -> terl::Result<Self, terl::ParseError> {
        let name = p.parse(Ident::parse)?;
        skip_whitespace(p);
        parse_char(p, '=')?;
        let value = p.parse(Ident::parse)?;
        Ok(Define { name, value })
    }
}

pub fn parse_args(p: &mut Parser<char>) -> terl::Result<Vec<Ident>, terl::ParseError> {
    let mut args = Vec::new();
    while let Some(ident) = p.try_match(Ident::parse)? {
        args.push(ident);
    }
    Ok(args)
}

fn parse_tab(p: &mut Parser<char>) -> terl::Result<(), terl::ParseError> {
    let unmatch = "expect one of `\t` or `    `";
    p.r#try(|p: &mut Parser<char>| {
        if p.next().is_some_and(|c| *c == '\t') {
            Ok(())
        } else {
            p.unmatch(unmatch)
        }
    })
    .or_try(|p| {
        let take_white_space = || p.next_if(|c| c.is_whitespace()).copied();
        if core::iter::from_fn(take_white_space).take(4).count() == 4 {
            Ok(())
        } else {
            p.unmatch(unmatch)
        }
    })
    .finish()
}

#[derive(Debug)]
pub struct Calling {
    pub called: Ident,
    pub args: Vec<Ident>,
}

impl Calling {
    fn parse(p: &mut Parser<char>) -> terl::Result<Self, terl::ParseError> {
        let called = p.parse(Ident::parse)?;
        let args = p.parse(parse_args)?;
        Ok(Calling { called, args })
    }
}

#[derive(Debug)]
pub struct Macro {
    pub called: Ident,
    pub args: Vec<Ident>,
}

impl Macro {
    fn parse(p: &mut Parser<char>) -> terl::Result<Self, terl::ParseError> {
        parse_char(p, '#')?;
        let called = p.parse(Ident::parse)?;
        let args = p.parse(parse_args)?;
        Ok(Macro { called, args })
    }
}

#[derive(Debug)]
pub enum Stmt {
    Calling(Calling),
    Macro(Macro),
}

impl Stmt {
    fn parse(p: &mut Parser<char>) -> terl::Result<Self, terl::ParseError> {
        terl::Try::<Stmt, char>::new(p)
            .or_try(|p| p.parse(Macro::parse).map(Stmt::Macro))
            .or_try(|p| p.parse(Calling::parse).map(Stmt::Calling))
            .finish()
    }
}

fn parse_stmts(p: &mut Parser<char>) -> terl::Result<Vec<Stmt>, terl::ParseError> {
    let mut stmts = Vec::new();
    let parse_one_stmt = |p: &mut Parser<char>| {
        p.parse(parse_tab)?;
        let stmt = p.parse(Stmt::parse)?;
        let term = p.parse(parse_eol)?;

        Ok((stmt, term))
    };
    while let Some((stmt, _term)) = p.try_match(parse_one_stmt)? {
        stmts.push(stmt);
    }
    Ok(stmts)
}

#[derive(Debug)]
pub struct Function {
    pub name: Ident,
    pub args: Vec<Ident>,
    pub body: Vec<Stmt>,
}

#[derive(Debug)]
pub enum Item {
    Define(Define),
    Function(Function),
    Calling(Calling),
    Macro(Macro),
}

impl Item {
    fn parse(p: &mut Parser<char>) -> terl::Result<Self, terl::ParseError> {
        terl::Try::<Item, char>::new(p)
            .or_try(|p| {
                let define = p.parse(Define::parse).map(Item::Define)?;
                p.parse(parse_eol)?;
                Ok(define)
            })
            .or_try(|p| {
                let macro_ = p.parse(Macro::parse).map(Item::Macro)?;
                p.parse(parse_eol)?;
                Ok(macro_)
            })
            .or_try(|p| {
                let name = p.parse(Ident::parse)?;
                let args = p.parse(parse_args)?;
                terl::Try::<Item, char>::new(p)
                    .or_try(|p| {
                        skip_whitespace(p);
                        parse_char(p, '=')?;
                        p.parse(parse_eol)?;
                        let commands = p.parse(parse_stmts)?;
                        Ok(Item::Function(Function {
                            name: name.clone(),
                            args: args.clone(),
                            body: commands,
                        }))
                    })
                    .or_try(|p| {
                        p.parse(parse_eol)?;
                        Ok(Item::Calling(Calling { called: name, args }))
                    })
                    .finish()
            })
            .or_try(|p| p.throw("invalid syntax"))
            .finish()
    }
}

pub fn parse_items(p: &mut Parser<char>) -> terl::Result<Vec<Item>, terl::ParseError> {
    let mut items = Vec::new();
    while p.peek().is_some() {
        if let Ok(term) = p.parse(parse_eol) {
            if term {
                break;
            }
            continue;
        }
        items.push(p.parse(Item::parse)?);
    }
    Ok(items)
}
