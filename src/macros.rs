use std::{collections::HashMap, sync::LazyLock};

use crate::{parser::Ident, Memory, Value};

#[derive(Debug)]
pub struct Meta {
    pub id: Ident,
    pub val: Option<Value>,
}

#[derive(Debug, Clone, Copy)]
pub struct Macro(fn(mem: &mut Memory, &[Meta]) -> Result<(), terl::Error>);

impl Macro {
    pub fn call(&self, mem: &mut Memory, metas: &[Meta]) -> Result<(), terl::Error> {
        self.0(mem, metas)
    }
}

fn print_mem(_mem: &mut Memory, metas: &[Meta]) -> Result<(), terl::Error> {
    for arg in metas.iter() {
        let val = arg.val.map(|v| _mem.read(v));
        println!("{}: {:?}", arg.id, val);
    }
    Ok(())
}

pub static MACROS: LazyLock<HashMap<&'static str, Macro>> =
    LazyLock::new(|| [("print_mem", Macro(print_mem))].into());
