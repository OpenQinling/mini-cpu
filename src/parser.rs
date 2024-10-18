use std::sync::Arc;

use terl::{mapper, ParseUnit, Parser, ResultMapperExt, Span, WithSpan, WithSpanExt};

#[derive(Debug)]
struct SkipSpace;

impl terl::ParseUnit<char> for SkipSpace {
    type Target = SkipSpace;

    fn parse(p: &mut Parser<char>) -> terl::Result<Self::Target, terl::ParseError> {
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
    type Target = bool;

    fn parse(p: &mut Parser<char>) -> terl::Result<Self::Target, terl::ParseError> {
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

#[derive(Debug)]
struct Tab;

impl terl::ReverseParseUnit<char> for Tab {
    type Left = ();

    fn reverse_parse(&self, p: &mut Parser<char>) -> terl::Result<Self::Left, terl::ParseError> {
        while let Some(next) = p.next() {
            match *next {
                '\t' => return Ok(()),
                _ if next.is_whitespace() => continue,
                _ => p.unmatch("expect TAB")?,
            }
        }
        p.unmatch("expect TAB")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ident(pub Arc<String>, Span);

impl WithSpan for Ident {
    fn get_span(&self) -> Span {
        self.1
    }
}

impl core::fmt::Display for Ident {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::ops::Deref for Ident {
    type Target = Arc<String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl terl::ParseUnit<char> for Ident {
    type Target = Self;

    fn parse(p: &mut Parser<char>) -> terl::Result<Self::Target, terl::ParseError> {
        _ = SkipSpace::parse(p);
        p.start_taking();
        let mut ident = String::new();
        while let Some(c) = p.next_if(|c| !c.is_whitespace() && *c != '=' && *c != ';') {
            ident.push(*c);
        }

        if ident.is_empty() {
            p.unmatch("no more ident")?;
        }

        Ok(Ident(ident.into(), p.get_span()))
    }
}

#[derive(Debug)]
pub struct Define {
    pub name: Ident,
    pub value: Ident,
}

impl terl::ParseUnit<char> for Define {
    type Target = Self;

    fn parse(p: &mut Parser<char>) -> terl::Result<Self::Target, terl::ParseError> {
        let name = Ident::parse(p)?;
        p.r#match(Equal)?;
        let value = Ident::parse(p)?;
        Ok(Define { name, value })
    }
}

#[derive(Debug)]
struct Args;

impl terl::ParseUnit<char> for Args {
    type Target = Vec<Ident>;

    fn parse(p: &mut Parser<char>) -> terl::Result<Self::Target, terl::ParseError> {
        let mut args = Vec::new();
        while let Some(ident) = Ident::parse(p).apply(mapper::Try)? {
            args.push(ident);
        }
        Ok(args)
    }
}

#[derive(Debug)]
struct Commands;

impl terl::ParseUnit<char> for Commands {
    type Target = Vec<Command>;

    fn parse(p: &mut Parser<char>) -> terl::Result<Self::Target, terl::ParseError> {
        let once = |p: &mut Parser<char>| {
            p.r#match(Tab)?;
            Command::parse(p)
        };

        let mut commands = Vec::new();
        while let Some(command) = p.once(once).apply(mapper::Try)? {
            commands.push(command);

            if p.r#match(Eol).apply(mapper::MustMatch)? {
                return Ok(commands);
            }
        }
        Ok(commands)
    }
}

#[derive(Debug)]
pub struct Command {
    pub called: Ident,
    pub args: Vec<Ident>,
}

impl terl::ParseUnit<char> for Command {
    type Target = Self;

    fn parse(p: &mut Parser<char>) -> terl::Result<Self::Target, terl::ParseError> {
        let called = Ident::parse(p)?;
        let args = Args::parse(p)?;
        Ok(Command { called, args })
    }
}

#[derive(Debug)]
pub struct Function {
    pub name: Ident,
    pub args: Vec<Ident>,
    pub body: Vec<Command>,
}

#[derive(Debug)]
pub struct MacroCall {
    pub called: Ident,
    pub args: Vec<Ident>,
}

impl terl::ParseUnit<char> for MacroCall {
    type Target = MacroCall;

    fn parse(p: &mut Parser<char>) -> terl::Result<Self::Target, terl::ParseError> {
        if p.next_if(|c| *c == '#').is_none() {
            return p.unmatch("expect '#'");
        }
        let called = Ident::parse(p)?;
        let args = Args::parse(p)?;
        Ok(MacroCall { called, args })
    }
}

#[derive(Debug)]
pub enum Item {
    Define(Define),
    Function(Function),
    Command(Command),
    MacroCall(MacroCall),
}

impl terl::ParseUnit<char> for Item {
    type Target = Self;

    fn parse(p: &mut Parser<char>) -> terl::Result<Self::Target, terl::ParseError> {
        terl::Try::<Item, char>::new(p)
            .or_try::<Item, _>(|p| {
                let define = Define::parse(p).map(Item::Define)?;
                p.r#match(Eol).apply(mapper::MustMatch)?;
                Ok(define)
            })
            .or_try::<Item, _>(|p| {
                let macro_ = MacroCall::parse(p).map(Item::MacroCall)?;
                p.r#match(Eol).apply(mapper::MustMatch)?;
                Ok(macro_)
            })
            .or_try::<Item, _>(|p| {
                let name = Ident::parse(p)?;
                let args = Args::parse(p)?;
                terl::Try::<Item, char>::new(p)
                    .or_try::<Item, _>(|p| {
                        p.r#match(Equal)?;
                        p.r#match(Eol).apply(mapper::MustMatch)?;
                        let commands = Commands::parse(p)?;
                        Ok(Item::Function(Function {
                            name: name.clone(),
                            args: args.clone(),
                            body: commands,
                        }))
                    })
                    .or_try::<Item, _>(|p| {
                        p.r#match(Eol).apply(mapper::MustMatch)?;
                        Ok(Item::Command(Command { called: name, args }))
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
    type Target = Vec<Item>;

    fn parse(p: &mut Parser<char>) -> terl::Result<Self::Target, terl::ParseError> {
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
