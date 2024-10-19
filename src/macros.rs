use std::{
    collections::HashMap,
    sync::{Arc, LazyLock},
};

use terl::{Error, FileBuffer, MakeError};

use crate::{compiler::Compiler, parser::Ident, Memory, Value};

#[derive(Debug)]
pub struct Meta {
    pub id: Ident,
    pub val: Option<Value>,
}

pub type VirtualCall = fn(mem: &mut Memory, metas: &[Meta]) -> Result<(), Error>;
pub type Preprocess = fn(c: &mut Compiler, args: &[Ident]) -> Result<(), Error>;

#[derive(Debug, Clone, Copy)]
pub enum Macro {
    Fn(VirtualCall),
    Preprocess(Preprocess),
}

impl Macro {}

fn print_mem(_mem: &mut Memory, metas: &[Meta]) -> Result<(), Error> {
    for arg in metas.iter() {
        let val = arg.val.map(|v| _mem.read(v));
        println!("{}: {:?}", arg.id, val);
    }
    Ok(())
}

fn include(c: &mut Compiler, args: &[Ident]) -> Result<(), Error> {
    for file_name in args {
        let source = std::fs::read_to_string(file_name.literal().as_str()).map_err(|e| {
            let reason = format!("failed to read file `{}`: {}", file_name.literal(), e);
            file_name.make_error(reason)
        })?;
        let file_buffer = Arc::new(FileBuffer::new(
            file_name.literal().to_owned(),
            source.chars().collect(),
        ));
        c.compile_file(file_buffer)?;
    }
    Ok(())
}

pub static MACROS: LazyLock<HashMap<&'static str, Macro>> = LazyLock::new(|| {
    HashMap::from([
        ("print_mem", Macro::Fn(print_mem)),
        ("include", Macro::Preprocess(include)),
    ])
});
