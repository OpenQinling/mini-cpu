use std::{collections::HashMap, sync::Arc};

use terl::{AsBuffer, Error, FileBuffer, MakeError, WithBufName, WithSpan};

use crate::{
    macros,
    parser::{self, Ident, Stmt},
};

#[derive(Debug)]
struct Function {
    name: Ident,
    args: Vec<Ident>,
    body: Vec<Stmt>,
}

#[derive(Debug)]
struct MacroCall {
    called: macros::VirtualCall,
    args: Vec<macros::Meta>,
}

impl MacroCall {
    fn call(&self, memory: &mut crate::Memory) -> Result<(), Error> {
        (self.called)(memory, &self.args)
    }
}

#[derive(Debug)]
enum Command {
    Command(crate::Command),
    MacroCall(MacroCall),
}

impl From<crate::Command> for Command {
    fn from(v: crate::Command) -> Self {
        Self::Command(v)
    }
}

impl From<MacroCall> for Command {
    fn from(v: MacroCall) -> Self {
        Self::MacroCall(v)
    }
}

#[derive(Debug, Default)]
pub struct Compiler {
    // for defines
    defines: HashMap<Arc<str>, crate::Value>,
    // for function
    args: HashMap<Arc<str>, crate::Value>,
    functions: HashMap<Arc<str>, Arc<Function>>,

    files: HashMap<Arc<str>, Arc<FileBuffer>>,

    commands: Vec<Command>,
}

impl Compiler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn compile_file(&mut self, buffer: Arc<FileBuffer>) -> Result<(), Error> {
        self.files
            .insert(buffer.buf_name().to_owned(), buffer.clone());
        let mut parser = terl::Parser::new(buffer.clone());

        let items = parser.parse::<parser::Items>().map_err(|e| {
            let msg = format!("faild to compile file {}", buffer.buf_name());
            let mut error = Error::from(terl::Message::text(msg, buffer.buf_name().to_owned()));
            error.extend(e.error().into_mesages());
            error
        })?;

        for item in items {
            self.compile_item(item)?;
        }

        Ok(())
    }

    pub fn compile_define(&mut self, define: parser::Define) -> Result<(), Error> {
        let value = self.redirect(&define.value)?;
        self.defines.insert(define.name.literal().clone(), value);
        Ok(())
    }

    pub fn compile_function(&mut self, function: parser::Function) -> Result<(), Error> {
        if let Some((.., exist)) = self.functions.get_key_value(function.name.literal()) {
            let message = exist.name.make_message("function already exists");
            let err = function.name.make_error("function already exists");
            return Err(err.append(message));
        }

        let name = function.name;
        let args = function.args;
        let body = function.body;

        self.functions.insert(
            name.literal().to_owned(),
            Arc::new(Function { name, args, body }),
        );

        Ok(())
    }

    pub fn redirect<'a>(&'a self, value: &'a Ident) -> Result<crate::Value, Error> {
        self.args
            .get(value.literal())
            .or_else(|| self.defines.get(value.literal()))
            .copied()
            .map(Ok)
            .unwrap_or_else(|| value.literal().parse::<crate::Value>())
            .map_err(|e| {
                let reason = format!("{}: {value}", e);
                value.make_error(reason)
            })
    }

    pub fn compile_calling(&mut self, calling: &parser::Calling) -> Result<(), Error> {
        let buf_name = calling.called.buf_name();
        let called_span = calling.called.get_span();
        let args_span = calling
            .args
            .iter()
            .map(WithSpan::get_span)
            .reduce(|l, r| l + r)
            .unwrap_or(called_span);

        match calling.called.literal().as_str() {
            builtin if builtin.parse::<crate::Op>().is_ok() => {
                if calling.args.len() != 2 {
                    let reason = format!("{} call requires 2 arguments", builtin);
                    return Err(Error::new(args_span, buf_name.to_owned(), reason));
                }
                let a = self.redirect(&calling.args[0])?;
                let b = self.redirect(&calling.args[1])?;
                self.commands
                    .push(crate::Command::new(builtin.parse().unwrap(), a, b).into());
            }
            _custom => {
                let fn_called = &calling.called;
                if let Some(function) = self.functions.get(calling.called.literal()).cloned() {
                    if function.args.len() != calling.args.len() {
                        let reason = format!(
                            "function {} requires {} arguments",
                            fn_called.literal(),
                            function.args.len()
                        );
                        return Err(Error::new(args_span, buf_name.to_owned(), reason));
                    }

                    let mut conflicts = HashMap::new();

                    for (arg, literal) in function.args.iter().zip(calling.args.iter()) {
                        let arg = arg.literal();
                        let value = self.redirect(literal)?;
                        if let Some(conflict) = self.args.insert(arg.clone(), value) {
                            conflicts.insert(arg.clone(), conflict);
                        }
                    }

                    for stmt in &function.body {
                        self.compile_stmt(stmt)?;
                    }

                    for arg in &function.args {
                        self.args.remove(arg.literal());
                    }

                    for (arg, value) in conflicts {
                        self.args.insert(arg, value);
                    }
                } else {
                    let undefined = format!("undefined function {}", fn_called.literal());
                    return Err(Error::new(args_span, buf_name.to_owned(), undefined));
                }
            }
        }

        Ok(())
    }

    pub fn compile_macro(&mut self, r#macro: &parser::Macro) -> Result<(), Error> {
        let Some(macro_) = macros::MACROS
            .get(r#macro.called.literal().as_str())
            .copied()
        else {
            let reason = format!("undefined macro {}", r#macro.called);
            return Err(r#macro.called.make_error(reason));
        };

        match macro_ {
            macros::Macro::Preprocess(preprocess) => preprocess(self, &r#macro.args),
            macros::Macro::Fn(vf) => {
                let make_meta = |arg: &Ident| {
                    let val = self.redirect(arg).ok();
                    let id = arg.to_owned();
                    macros::Meta { id, val }
                };
                let metas = r#macro.args.iter().map(make_meta).collect();
                let call = MacroCall {
                    called: vf,
                    args: metas,
                };

                self.commands.push(call.into());

                Ok(())
            }
        }
    }

    pub fn compile_stmt(&mut self, stmt: &parser::Stmt) -> Result<(), Error> {
        match stmt {
            Stmt::Calling(calling) => self.compile_calling(calling),
            Stmt::Macro(r#macro) => self.compile_macro(r#macro),
        }
    }

    pub fn compile_item(&mut self, item: parser::Item) -> Result<(), Error> {
        match item {
            parser::Item::Define(define) => self.compile_define(define)?,
            parser::Item::Function(function) => self.compile_function(function)?,
            parser::Item::Calling(calling) => self.compile_calling(&calling)?,
            parser::Item::Macro(r#macro) => self.compile_macro(&r#macro)?,
        }
        Ok(())
    }

    pub fn handle_error(&self, error: &Error) -> Result<String, String> {
        let mut output = String::new();
        for message in error.messages() {
            let buf_name = message.buf_name();
            let buffer = self
                .files
                .get(buf_name)
                .ok_or_else(|| format!("buffer not found: {}", buf_name))?;
            <char as terl::Source>::handle_message(buffer.as_ref().as_ref(), &mut output, message)
                .map_err(|e| e.to_string())?;
        }

        Ok(output)
    }

    pub fn commands(&self) -> impl Iterator<Item = &crate::Command> {
        self.commands.iter().filter_map(|c| match c {
            Command::Command(c) => Some(c),
            Command::MacroCall(_) => None,
        })
    }

    pub fn run(&mut self, pc_val: crate::Value, memory: &mut crate::Memory) {
        let mut macros = HashMap::new();
        self.commands
            .iter_mut()
            .fold(pc_val, |pc_val, command| match command {
                Command::Command(c) => {
                    c.encode(&mut memory[pc_val.0 as usize..]);
                    pc_val.next_command().unwrap()
                }
                Command::MacroCall(m) => {
                    macros.entry(pc_val).or_insert_with(Vec::new).push(m);
                    pc_val
                }
            });

        let mut pc_val = pc_val;
        loop {
            memory.write(0x00.into(), pc_val);
            if let Some(macros) = macros.get(&pc_val) {
                for m in macros {
                    m.call(memory).unwrap();
                }
            }
            if memory.eval(0x00.into()).is_err() {
                println!("Aborted");
                break;
            }

            pc_val = pc_val.next_command().unwrap();
        }
    }
}
