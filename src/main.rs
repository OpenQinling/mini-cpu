#![feature(str_as_str)]
pub mod compiler;
pub mod macros;
pub mod parser;

use std::{
    num::ParseIntError,
    ops::{Deref, DerefMut},
    str::FromStr,
    sync::Arc,
};

#[derive(Debug, Clone, Copy)]
enum Op {
    Unknow1,
    Sub,
    Set,
    Unknow2,
    Lod,
    Str,
}

impl Op {
    fn execute(&self, mem: &mut Memory, a: Value, b: Value) {
        match self {
            // *a += *b
            Op::Unknow1 => {
                unreachable!("Unknow1");
            }
            // *a -= *b
            Op::Sub => {
                let tmp = mem.read(a);
                let (result, _) = tmp.0.overflowing_sub(mem.read(b).0);
                mem.write(a, result.into());
            }
            // *a = b
            Op::Set => {
                mem.write(a, b);
            }

            // *b = *a
            Op::Unknow2 => {
                unreachable!()
            }
            // *a = **b
            Op::Lod => {
                let ptr = mem.read(b);
                let data = mem.read(ptr);
                mem.write(a, data);
            }
            // **b = *a
            Op::Str => {
                let data = mem.read(a);
                let ptr = mem.read(b);

                mem.write(ptr, data);
            }
        }
        // *c = a
        // b = *c
    }
}

#[derive(Debug)]
pub struct InvalidOp;

impl FromStr for Op {
    type Err = InvalidOp;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            // "Unknow1" => Ok(Op::Unknow1),
            "SUB" => Ok(Op::Sub),
            "SET" => Ok(Op::Set),
            // "Unknow2" => Ok(Op::Unknow2),
            "LOD" => Ok(Op::Lod),
            "STR" => Ok(Op::Str),
            _ => Err(InvalidOp),
        }
    }
}

impl TryFrom<u8> for Op {
    type Error = InvalidOp;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Op::Unknow1),
            2 => Ok(Op::Sub),
            3 => Ok(Op::Set),
            4 => Ok(Op::Unknow2),
            5 => Ok(Op::Lod),
            6 => Ok(Op::Str),
            _ => Err(InvalidOp),
        }
    }
}

impl From<Op> for u8 {
    fn from(value: Op) -> Self {
        match value {
            Op::Unknow1 => 1,
            Op::Sub => 2,
            Op::Set => 3,
            Op::Unknow2 => 4,
            Op::Lod => 5,
            Op::Str => 6,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Command {
    op: Op,
    a: Value,
    b: Value,
}

impl Command {
    fn new(op: Op, a: Value, b: Value) -> Command {
        Command { op, a, b }
    }

    fn execute(&self, mem: &mut Memory) {
        self.op.execute(mem, self.a, self.b)
    }

    fn encode(&self, memory: &mut [u8]) {
        assert!(memory.len() >= 5);
        // 1 + 2 + 2 = 5bytes
        memory[0] = self.op.into();
        memory[1..3].copy_from_slice(&self.a.to_le_bytes());
        memory[3..5].copy_from_slice(&self.b.to_le_bytes());
    }

    fn decode(memory: &[u8]) -> Result<Command, InvalidOp> {
        assert!(memory.len() >= 5);
        let operator = Op::try_from(memory[0])?;
        let a = Value(u16::from_le_bytes(memory[1..3].try_into().unwrap()));
        let b = Value(u16::from_le_bytes(memory[3..5].try_into().unwrap()));
        Ok(Command::new(operator, a, b))
    }
}

trait Encode {
    fn encode(&mut self, memory: &mut [u8]);
}

impl Encode for &Command {
    fn encode(&mut self, memory: &mut [u8]) {
        Command::encode(self, memory);
    }
}

impl<E, I> Encode for I
where
    E: Encode,
    I: Iterator<Item = E>,
{
    fn encode(&mut self, mut memory: &mut [u8]) {
        for mut command in self {
            command.encode(&mut memory[..5]);
            memory = &mut memory[5..];
        }
    }
}

pub struct Memory<'m> {
    memory: &'m mut [u8],
}

impl Memory<'_> {
    fn new(memory: &mut [u8]) -> Memory {
        Memory { memory }
    }

    fn eval(&mut self, pc: Value) -> Result<(), InvalidOp> {
        let pc_val = self.read(pc);
        let command = Command::decode(&self.memory[pc_val.0 as usize..])?;
        println!("eval: {command:?} at {}", pc_val.0);
        command.execute(self);
        Ok(())
    }

    fn read(&self, ptr: Value) -> Value {
        let index = ptr.0 as usize;

        let memory: [u8; 2] = self.memory[index..index + 2].try_into().unwrap();
        Value(u16::from_le_bytes(memory))
    }

    fn write(&mut self, ptr: Value, value: Value) {
        let (Value(ptr), Value(value)) = (ptr, value);
        self.memory[ptr as usize..ptr as usize + 2].copy_from_slice(&value.to_le_bytes());
    }
}

impl<'m> From<&'m mut [u8]> for Memory<'m> {
    fn from(memory: &'m mut [u8]) -> Self {
        Memory { memory }
    }
}

impl Deref for Memory<'_> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.memory
    }
}

impl DerefMut for Memory<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.memory
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Value(u16);

impl core::fmt::Display for Value {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "0x{:04x}", self.0)
    }
}

#[derive(Debug, Clone, Copy)]
struct Aborted;

impl Value {
    const fn new(value: u16) -> Value {
        Value(value)
    }

    fn next_command(&self) -> Result<Value, Aborted> {
        self.0.checked_add(5).map(Value).ok_or(Aborted)
    }
}

impl Deref for Value {
    type Target = u16;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Value {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<u16> for Value {
    fn from(value: u16) -> Self {
        Value(value)
    }
}

impl FromStr for Value {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(str) = s.strip_prefix("0x") {
            u16::from_str_radix(str, 16).map(Value)
        } else if let Some(str) = s.strip_prefix("0b") {
            u16::from_str_radix(str, 2).map(Value)
        } else if let Some(str) = s.strip_prefix("0o") {
            u16::from_str_radix(str, 8).map(Value)
        } else {
            u16::from_str(s).map(Value)
        }
    }
}

fn main() {
    let mut memory = [0u8; 65536];
    let mut memory = Memory::new(&mut memory);

    let src = std::fs::read_to_string("code.mc").unwrap();
    let buffer = terl::FileBuffer::new("code.mc".into(), src.chars().collect());
    let buffer = Arc::new(buffer);

    let mut compiler = compiler::Compiler::new();
    compiler
        .compile_file(buffer.clone())
        .map_err(|e| compiler.handle_error(&e).unwrap())
        .map_err(|e| println!("{e}"))
        .unwrap();

    compiler.run(Value::new(0xf000), &mut memory);
}
