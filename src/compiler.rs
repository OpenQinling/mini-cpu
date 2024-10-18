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

impl MacroCall {
    fn call(&self, memory: &mut crate::Memory) -> Result<(), terl::Error> {
        self.called.call(memory, &self.args)
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
    defines: HashMap<Arc<String>, crate::Value>,
    // for function
    args: HashMap<Arc<String>, crate::Value>,
    functions: HashMap<Arc<String>, Arc<Function>>,

    commands: Vec<Command>,
}

impl Compiler {
    pub fn compile_define(&mut self, define: parser::Define) -> Result<(), terl::Error> {
        let value = self.parse_value(&define.value)?;
        self.defines.insert(define.name.0, value);
        Ok(())
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

    pub fn parse_value<'a>(&'a self, value: &'a Ident) -> Result<crate::Value, terl::Error> {
        self.args
            .get(value.deref())
            .or_else(|| self.defines.get(value.deref()))
            .copied()
            .map(Ok)
            .unwrap_or_else(|| value.parse::<crate::Value>())
            .map_err(|e| {
                let reason = format!("{}: {value}", e);
                value.make_error(reason)
            })
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
            _custom => {
                let fn_called = &command.called;
                if let Some(function) = self.functions.get(command.called.deref()).cloned() {
                    if function.args.len() != command.args.len() {
                        let reason = format!(
                            "function {} requires {} arguments",
                            fn_called.as_str(),
                            function.args.len()
                        );
                        return Err(args_span.make_error(reason));
                    }

                    let mut conflict = HashMap::new();

                    for (arg, value) in function.args.iter().zip(command.args.iter()) {
                        let value = self.parse_value(value).to_owned()?;
                        if let Some(conf) = self.args.insert(arg.deref().clone(), value) {
                            conflict.insert(arg.deref().clone(), conf);
                        }
                    }

                    for command in &function.body {
                        self.compile_command(command)?;
                    }

                    for arg in &function.args {
                        self.args.remove(arg.deref());
                    }

                    for (arg, value) in conflict {
                        self.args.insert(arg, value);
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
            parser::Item::Define(define) => self.compile_define(define)?,
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
