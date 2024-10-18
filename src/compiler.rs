use std::{collections::HashMap, ops::Deref, sync::Arc};

use terl::{Span, WithSpan};

use crate::{
    macros,
    parser::{self, Ident},
};

#[derive(Debug)]
struct Function {
    at: Span,
    args: Vec<Ident>,
    body: Vec<parser::Command>,
}

#[derive(Debug)]
struct MacroCall {
    called: macros::Macro,
    args: Vec<macros::Meta>,
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
    defines: HashMap<Arc<String>, Ident>,
    // for function
    args: HashMap<Arc<String>, Ident>,
    functions: HashMap<Arc<String>, Arc<Function>>,

    commands: Vec<Command>,
}

impl Compiler {
    pub fn compile_define(&mut self, define: parser::Define) {
        self.defines.insert(define.name.0, define.value);
    }

    pub fn compile_function(&mut self, function: parser::Function) -> Result<(), terl::Error> {
        if let Some((.., exist)) = self.functions.get_key_value(function.name.deref()) {
            let message = exist.at.make_message("function already exists");
            let err = function.name.make_error("function already exists");
            return Err(err.append(message));
        }

        let at = function.name.get_span();
        let args = function.args;
        let body = function.body;
        let name = function.name.0;

        self.functions
            .insert(name, Arc::new(Function { at, args, body }));

        Ok(())
    }

    pub fn true_name<'a>(&'a self, value: &'a Ident) -> &'a Ident {
        self.args
            .get(value.deref())
            .or_else(|| self.defines.get(value.deref()))
            .unwrap_or(value)
    }

    pub fn parse_value(&self, ident: &parser::Ident) -> Result<crate::Value, terl::Error> {
        let true_name = self.true_name(ident);
        true_name
            .parse::<crate::Value>()
            .map_err(|e| ident.make_error(e))
    }

    pub fn compile_command(&mut self, command: &parser::Command) -> Result<(), terl::Error> {
        let called_span = command.called.get_span();
        let args_span = command
            .args
            .iter()
            .map(WithSpan::get_span)
            .reduce(|l, r| l + r)
            .unwrap_or(called_span);

        match command.called.as_str() {
            builtin if builtin.parse::<crate::Op>().is_ok() => {
                if command.args.len() != 2 {
                    return Err(
                        args_span.make_error(format!("{} command requires 2 arguments", builtin))
                    );
                }
                let a = self.parse_value(&command.args[0])?;
                let b = self.parse_value(&command.args[1])?;
                self.commands
                    .push(crate::Command::new(builtin.parse().unwrap(), a, b).into());
            }
            custom => {
                let fn_called = self.true_name(&command.called);
                if let Some(function) = self.functions.get(fn_called.deref()).cloned() {
                    if function.args.len() != command.args.len() {
                        let reason = format!(
                            "function {} requires {} arguments",
                            custom,
                            function.args.len()
                        );
                        return Err(args_span.make_error(reason));
                    }

                    for (arg, value) in function.args.iter().zip(command.args.iter()) {
                        let value = self.true_name(value).to_owned();
                        self.args.insert(arg.deref().clone(), value);
                    }

                    for command in &function.body {
                        self.compile_command(command)?;
                    }
                } else {
                    let undefined = format!("undefined function {}", fn_called.as_str());
                    return Err(fn_called.make_error(undefined));
                }
            }
        }

        Ok(())
    }

    pub fn compile_macro_call(&mut self, macro_call: parser::MacroCall) -> Result<(), terl::Error> {
        let Some(macro_) = macros::MACROS.get(macro_call.called.as_str()).copied() else {
            let reason = format!("undefined macro {}", macro_call.called);
            return Err(macro_call.called.make_error(reason));
        };

        let metas = macro_call
            .args
            .iter()
            .map(|arg| macros::Meta {
                id: arg.to_owned(),
                tn: self.true_name(arg).to_owned(),
                val: self.parse_value(arg).ok(),
            })
            .collect();
        let call = MacroCall {
            called: macro_,
            args: metas,
        };

        self.commands.push(call.into());

        Ok(())
    }

    pub fn compile_item(&mut self, item: parser::Item) -> Result<(), terl::Error> {
        match item {
            parser::Item::Define(define) => self.compile_define(define),
            parser::Item::Function(function) => self.compile_function(function)?,
            parser::Item::Command(command) => self.compile_command(&command)?,
            parser::Item::MacroCall(macto_call) => self.compile_macro_call(macto_call)?,
        }
        Ok(())
    }

    pub fn commands(&self) -> impl Iterator<Item = &crate::Command> {
        self.commands.iter().filter_map(|c| match c {
            Command::Command(c) => Some(c),
            Command::MacroCall(_) => None,
        })
    }

    pub fn run(&self, mut pc: crate::Value, memory: &mut crate::Memory) {
        // 1. load command from internal memory
    }
}
