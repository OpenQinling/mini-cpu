use std::sync::Arc;

use terl::{mapper, MakeError, ParseUnit, Parser, ResultMapperExt, Span, WithSpan};

#[derive(Debug)]
struct SkipSpace;

impl terl::ParseUnit<char> for SkipSpace {
    type Result = SkipSpace;

    fn parse(p: &mut Parser<char>) -> terl::Result<Self::Result, terl::ParseError> {
        while p
            .next_if(|c| c.is_ascii_whitespace() && *c != '\n')
            .is_some()
        {}
        Ok(SkipSpace)
    }
}

#[derive(Debug)]
struct Equal;

impl terl::ReverseParseUnit<char> for Equal {
    type Left = Equal;

    fn reverse_parse(&self, p: &mut Parser<char>) -> terl::Result<Self::Left, terl::ParseError> {
        _ = SkipSpace::parse(p);
        if p.next_if(|c| *c == '=').is_none() {
            p.unmatch("expect '='")?;
        }
        Ok(Equal)
    }
}

#[derive(Debug)]
struct Comment;

impl terl::ParseUnit<char> for Comment {
    type Result = bool;

    fn parse(p: &mut Parser<char>) -> terl::Result<Self::Result, terl::ParseError> {
        if p.next_if(|c| *c == ';').is_none() {
            p.unmatch("expect ';'")?;
        }
        while let Some(next) = p.next() {
            if *next == '\n' {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

#[derive(Debug)]
struct Eol;

impl terl::ReverseParseUnit<char> for Eol {
    type Left = bool;

    fn reverse_parse(&self, p: &mut Parser<char>) -> terl::Result<Self::Left, terl::ParseError> {
        _ = SkipSpace::parse(p);

        if let Ok(term) = Comment::parse(p) {
            return Ok(term);
        }

        match p.next() {
            Some(&'\n') => Ok(false),
            None => Ok(true),
            _ => p.unmatch("expect EOL"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ident {
    literal: Arc<str>,
    buf_name: Arc<str>,
    location: Span,
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

impl terl::ParseUnit<char> for Ident {
    type Result = Self;

    fn parse(p: &mut Parser<char>) -> terl::Result<Self::Result, terl::ParseError> {
        _ = SkipSpace::parse(p);
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

#[derive(Debug)]
pub struct Define {
    pub name: Ident,
    pub value: Ident,
}

impl terl::ParseUnit<char> for Define {
    type Result = Self;

    fn parse(p: &mut Parser<char>) -> terl::Result<Self::Result, terl::ParseError> {
        let name = Ident::parse(p)?;
        p.r#match(Equal)?;
        let value = Ident::parse(p)?;
        Ok(Define { name, value })
    }
}

#[derive(Debug)]
struct Args;

impl terl::ParseUnit<char> for Args {
    type Result = Vec<Ident>;

    fn parse(p: &mut Parser<char>) -> terl::Result<Self::Result, terl::ParseError> {
        let mut args = Vec::new();
        while let Some(ident) = Ident::parse(p).apply(mapper::Try)? {
            args.push(ident);
        }
        Ok(args)
    }
}

#[derive(Debug)]
struct Tab;

impl terl::ParseUnit<char> for Tab {
    type Result = ();

    fn parse(p: &mut Parser<char>) -> terl::Result<Self::Result, terl::ParseError> {
        let unmatch = "expect one of `\t` or `    `";
        terl::Try::<Tab, char>::new(p)
            .or_try(|p| {
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
}

impl terl::ReverseParseUnit<char> for Tab {
    type Left = ();

    fn reverse_parse(&self, p: &mut Parser<char>) -> terl::Result<Self::Left, terl::ParseError> {
        p.parse::<Self>().map(|_| ())
    }
}

#[derive(Debug)]
pub struct Calling {
    pub called: Ident,
    pub args: Vec<Ident>,
}

impl terl::ParseUnit<char> for Calling {
    type Result = Self;

    fn parse(p: &mut Parser<char>) -> terl::Result<Self::Result, terl::ParseError> {
        let called = Ident::parse(p)?;
        let args = Args::parse(p)?;
        Ok(Calling { called, args })
    }
}

#[derive(Debug)]
pub struct Macro {
    pub called: Ident,
    pub args: Vec<Ident>,
}

impl terl::ParseUnit<char> for Macro {
    type Result = Macro;

    fn parse(p: &mut Parser<char>) -> terl::Result<Self::Result, terl::ParseError> {
        if p.next_if(|c| *c == '#').is_none() {
            return p.unmatch("expect '#'");
        }
        let called = Ident::parse(p)?;
        let args = Args::parse(p)?;
        Ok(Macro { called, args })
    }
}

#[derive(Debug)]
pub enum Stmt {
    Calling(Calling),
    Macro(Macro),
}

impl terl::ParseUnit<char> for Stmt {
    type Result = Self;

    fn parse(p: &mut Parser<char>) -> terl::Result<Self::Result, terl::ParseError> {
        terl::Try::<Stmt, char>::new(p)
            .or_try(|p| p.parse::<Calling>().map(Self::Calling))
            .or_try(|p| p.parse::<Macro>().map(Self::Macro))
            .finish()
    }
}

#[derive(Debug)]
struct Stmts;

impl terl::ParseUnit<char> for Stmts {
    type Result = Vec<Stmt>;

    fn parse(p: &mut Parser<char>) -> terl::Result<Self::Result, terl::ParseError> {
        let once = |p: &mut Parser<char>| {
            p.r#match(Tab)?;
            Stmt::parse(p)
        };

        let mut stmts = Vec::new();
        while let Some(stmt) = p.once(once).apply(mapper::Try)? {
            stmts.push(stmt);

            if p.r#match(Eol).apply(mapper::MustMatch)? {
                return Ok(stmts);
            }
        }
        Ok(stmts)
    }
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

impl terl::ParseUnit<char> for Item {
    type Result = Self;

    fn parse(p: &mut Parser<char>) -> terl::Result<Self::Result, terl::ParseError> {
        terl::Try::<Item, char>::new(p)
            .or_try(|p| {
                let define = Define::parse(p).map(Item::Define)?;
                p.r#match(Eol).apply(mapper::MustMatch)?;
                Ok(define)
            })
            .or_try(|p| {
                let macro_ = Macro::parse(p).map(Item::Macro)?;
                p.r#match(Eol).apply(mapper::MustMatch)?;
                Ok(macro_)
            })
            .or_try(|p| {
                let name = Ident::parse(p)?;
                let args = Args::parse(p)?;
                terl::Try::<Item, char>::new(p)
                    .or_try(|p| {
                        p.r#match(Equal)?;
                        p.r#match(Eol).apply(mapper::MustMatch)?;
                        let commands = Stmts::parse(p)?;
                        Ok(Item::Function(Function {
                            name: name.clone(),
                            args: args.clone(),
                            body: commands,
                        }))
                    })
                    .or_try(|p| {
                        p.r#match(Eol).apply(mapper::MustMatch)?;
                        Ok(Item::Calling(Calling { called: name, args }))
                    })
                    .or_error("invalid syntax")
                    .finish()
            })
            .or_error("invalid syntax")
            .finish()
    }
}

#[derive(Debug)]
pub struct Items;

impl terl::ParseUnit<char> for Items {
    type Result = Vec<Item>;

    fn parse(p: &mut Parser<char>) -> terl::Result<Self::Result, terl::ParseError> {
        let mut items = Vec::new();

        while p.peek().is_some() {
            if let Ok(term) = p.r#match(Eol) {
                if term {
                    break;
                }
                continue;
            }
            items.push(Item::parse(p).apply(mapper::MustMatch)?);
        }
        Ok(items)
    }
}
