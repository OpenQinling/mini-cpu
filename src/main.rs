// 16bit
// 2bit

// ADD A B -> A = A + B
// SUB A B -> A = A - B
// SET A B -> A = B
// CPY A B -> B = A
// LOD A B -> A = memory[B]

mod compiler;
mod macros;
mod parser;

use std::{
    num::ParseIntError,
    ops::{Deref, DerefMut},
    str::FromStr,
};

#[derive(Debug, Clone, Copy)]
enum Op {
    Add,
    Sub,
    Set,
    Cpy,
    Lod,
    Str,
}

impl Op {
    fn execute(&self, mem: &mut Memory, a: Value, b: Value) {
        match self {
            Op::Add => {
                let tmp = mem.read(a);
                let (result, _) = tmp.0.overflowing_add(mem.read(b).0);
                mem.write(a, result.into());
            }
            Op::Sub => {
                let tmp = mem.read(a);
                let (result, _) = tmp.0.overflowing_sub(mem.read(b).0);
                mem.write(a, result.into());
            }
            Op::Set => {
                mem.write(a, b);
            }
            Op::Cpy => {
                let a = mem.read(a);
                mem.write(b, a);
            }
            Op::Lod => {
                let ptr = mem.read(b);
                let data = mem.read(ptr);
                mem.write(a, data);
            }
            Op::Str => {
                let data = mem.read(a);
                let ptr = mem.read(b);
                mem.write(ptr, data);
            }
        }
    }
}

#[derive(Debug)]
pub struct InvalidOp;

impl FromStr for Op {
    type Err = InvalidOp;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ADD" => Ok(Op::Add),
            "SUB" => Ok(Op::Sub),
            "SET" => Ok(Op::Set),
            "CPY" => Ok(Op::Cpy),
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
            0 => Ok(Op::Add),
            1 => Ok(Op::Sub),
            2 => Ok(Op::Set),
            3 => Ok(Op::Cpy),
            4 => Ok(Op::Lod),
            5 => Ok(Op::Str),
            _ => Err(InvalidOp),
        }
    }
}

impl From<Op> for u8 {
    fn from(value: Op) -> Self {
        match value {
            Op::Add => 0,
            Op::Sub => 1,
            Op::Set => 2,
            Op::Cpy => 3,
            Op::Lod => 4,
            Op::Str => 5,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Command {
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
    fn encode(&mut self, memory: &mut [u8]) {
        let mut memory = memory;
        for mut command in self {
            command.encode(&mut memory[..5]);
            memory = &mut memory[5..];
        }
    }
}

struct Memory<'m> {
    memory: &'m mut [u8],
}

impl Memory<'_> {
    fn new(memory: &mut [u8]) -> Memory {
        Memory { memory }
    }

    fn eval(&mut self, pc: Value) -> Result<(), InvalidOp> {
        let pc = self.read(pc);
        let command = Command::decode(&self.memory[pc.0 as usize..])?;
        println!("eval: {command:?}");
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

#[derive(Debug, Clone, Copy)]
struct Value(u16);

impl core::fmt::Display for Value {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "0x{:04x}", self.0)
    }
}

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
    let buffer = terl::Buffer::new("code.mc".to_string(), src.chars().collect());
    let mut parser = terl::Parser::new(buffer);
    let items = parser.parse::<parser::Items>();

    let calling_tree = parser.calling_tree().to_string();
    let buffer = parser.take_buffer();

    let items = items
        .inspect_err(|_| println!("{calling_tree}"))
        .inspect_err(|e| println!("{:?}", e))
        .map_err(|e| <char as terl::Source>::handle_error(&buffer, e.error()))
        .map_err(|e| println!("{}", e))
        .unwrap();

    dbg!(&items);

    let mut compiler = compiler::Compiler::default();
    for item in items {
        compiler
            .compile_item(item)
            .inspect_err(|e| println!("{e:?}"))
            .map_err(|e| <char as terl::Source>::handle_error(&buffer, e))
            .unwrap();
    }

    let pc = Value::new(0xf000);
    let pc_at = Value::new(0xf000);
    memory.write(pc, pc_at);
    compiler.commands().encode(&mut memory[*pc as usize..]);

    // normal mode: real
    loop {
        memory.eval(pc).unwrap();
        if let Ok(next) = memory.read(pc).next_command() {
            memory.write(pc, next);
        } else {
            println!("aborted");
            break;
        }
    }
}
