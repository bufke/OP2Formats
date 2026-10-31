//! The stores of a function in Outpost2.exe, for the tables the game builds in code rather than keeping as data: each
//! unit type's `Init` fills its MapObjectType with animation ids (`units`), and a static initializer fills the cell
//! type table (0x40B5F0). A linear walk that knows the values of registers loaded with constants, which is all these
//! functions use: no branches are followed and nothing is executed.

use std::collections::BTreeMap;

use iced_x86::{Decoder, DecoderOptions, FlowControl, InstructionInfoFactory, Mnemonic, OpAccess, OpKind, Register};

use super::Exe;

/// What a function writes as constants.
#[derive(Debug, Default)]
pub struct Stores {
    /// Dwords written at `[this + offset]`, `this` being ECX on entry or a register copied from it before ECX is
    /// lost to a call.
    pub this_dwords: BTreeMap<u32, u32>,
    /// Bytes written at `[this + offset]`.
    pub this_bytes: BTreeMap<u32, u8>,
    /// Writes to fixed addresses: the value and its size in bytes.
    pub absolute: BTreeMap<u32, (u32, u8)>,
    /// Writes whose value the walk didn't know.
    pub unknown: usize,
}

/// Which bits of its 32-bit register an operand register is.
#[derive(Clone, Copy)]
enum Part {
    All,
    Low16,
    Low8,
    High8,
}

impl Part {
    fn of(self, v: u32) -> u32 {
        match self {
            Part::All => v,
            Part::Low16 => v & 0xFFFF,
            Part::Low8 => v & 0xFF,
            Part::High8 => (v >> 8) & 0xFF,
        }
    }
}

/// The 32-bit general register an operand register is part of (0 EAX .. 7 EDI), and which part.
fn gpr(r: Register) -> Option<(usize, Part)> {
    use Register as R;
    Some(match r {
        R::EAX => (0, Part::All),
        R::ECX => (1, Part::All),
        R::EDX => (2, Part::All),
        R::EBX => (3, Part::All),
        R::ESP => (4, Part::All),
        R::EBP => (5, Part::All),
        R::ESI => (6, Part::All),
        R::EDI => (7, Part::All),
        R::AX => (0, Part::Low16),
        R::CX => (1, Part::Low16),
        R::DX => (2, Part::Low16),
        R::BX => (3, Part::Low16),
        R::BP => (5, Part::Low16),
        R::SI => (6, Part::Low16),
        R::DI => (7, Part::Low16),
        R::AL => (0, Part::Low8),
        R::CL => (1, Part::Low8),
        R::DL => (2, Part::Low8),
        R::BL => (3, Part::Low8),
        R::AH => (0, Part::High8),
        R::CH => (1, Part::High8),
        R::DH => (2, Part::High8),
        R::BH => (3, Part::High8),
        _ => return None,
    })
}

/// A 32-bit general register other than ESP.
fn reg32(r: Register) -> Option<usize> {
    (r.size() == 4).then(|| gpr(r)).flatten().map(|(i, _)| i).filter(|&i| i != 4)
}

const ECX: usize = 1;

/// The stores of the function at `start` (at most `max_len` bytes). A register holds a known value after `mov reg,
/// imm`, `xor reg, reg` and `inc`, `dec`, `add` or `sub` of a constant on a known value; any other write to it or
/// to part of it (implicit ones too: `cdq`, `mul`, `rep movs`...), and any call, forgets it. Forward branches carry
/// what is known to their target, where it meets what falls through (a register stays known only if both agree); code
/// after a `ret` or jump starts from the branches to it alone, and code no branch reaches is skipped. The function
/// ends at a `ret` or a jump out of it (a tail call) that no branch seen so far jumps past, or at an `int3` after one
/// (the padding between MSVC's functions). `None` if the address isn't code in the file.
pub fn stores(exe: &Exe, start: u32, max_len: usize) -> Option<Stores> {
    let code = exe.code(start, max_len)?;
    let end = start as u64 + code.len() as u64;
    let mut out = Stores::default();
    let mut regs: [Option<u32>; 8] = [None; 8];
    let mut this = [false; 8];
    this[ECX] = true;
    let mut returned = false;
    // The furthest address a branch inside the function goes to: code up to there is still the function's.
    let mut reach = start as u64;
    let mut info = InstructionInfoFactory::new();
    // Forward branches not reached yet: their target and what was known when they were taken.
    let mut pending: Vec<(u64, [Option<u32>; 8], [bool; 8])> = Vec::new();
    // After a ret or a jump: until a branch target, nothing falls through.
    let mut dead = false;
    let value = |regs: &[Option<u32>; 8], r: Register| gpr(r).and_then(|(i, part)| regs[i].map(|v| part.of(v)));
    for ins in Decoder::with_ip(32, code, start as u64, DecoderOptions::NONE) {
        if ins.is_invalid() || (ins.mnemonic() == Mnemonic::Int3 && returned) {
            break;
        }
        // Where branches meet: what holds on every way in.
        let mut arrived = false;
        pending.retain(|&(t, r, th)| {
            if t != ins.ip() {
                return true;
            }
            if dead && !arrived {
                (regs, this) = (r, th);
            } else {
                for d in 0..8 {
                    regs[d] = regs[d].filter(|_| regs[d] == r[d]);
                    this[d] &= th[d];
                }
            }
            arrived = true;
            false
        });
        if dead && !arrived {
            continue;
        }
        dead = false;
        let branch = matches!(ins.op0_kind(), OpKind::NearBranch32).then(|| ins.near_branch_target());
        let inside = branch.filter(|&t| (start as u64..end).contains(&t));
        let exits = match ins.flow_control() {
            FlowControl::Return | FlowControl::IndirectBranch => true,
            FlowControl::UnconditionalBranch => inside.is_none(),
            _ => false,
        };
        if let Some(t) = inside
            && ins.flow_control() != FlowControl::Call
        {
            reach = reach.max(t);
            if t > ins.ip() {
                pending.push((t, regs, this));
            }
            if ins.flow_control() == FlowControl::UnconditionalBranch {
                dead = true;
                continue;
            }
        }
        if exits {
            if reach <= ins.ip() {
                break;
            }
            returned = true;
            dead = true;
            continue;
        }
        let m = ins.mnemonic();
        let (before, this_before) = (regs, this);
        // Every register the instruction writes, in whole or in part, is unknown after it unless a case below knows.
        for used in info.info(&ins).used_registers() {
            let writes = matches!(used.access(), OpAccess::Write | OpAccess::CondWrite | OpAccess::ReadWrite | OpAccess::ReadCondWrite);
            if let (true, Some((d, _))) = (writes, gpr(used.register())) {
                regs[d] = None;
                this[d] = false;
            }
        }
        let r0 = (ins.op_count() > 0 && ins.op0_kind() == OpKind::Register).then(|| ins.op0_register());
        let d0 = r0.and_then(reg32);
        let imm1 = ins.op_count() > 1
            && matches!(ins.op1_kind(), OpKind::Immediate8 | OpKind::Immediate16 | OpKind::Immediate32 | OpKind::Immediate8to32);
        match m {
            Mnemonic::Mov if ins.op_count() == 2 && ins.op0_kind() == OpKind::Memory => {
                let v = if imm1 {
                    Some(ins.immediate(1) as u32)
                } else if ins.op1_kind() == OpKind::Register {
                    value(&before, ins.op1_register())
                } else {
                    None
                };
                let (base, disp, size) = (ins.memory_base(), ins.memory_displacement32(), ins.memory_size().size());
                if ins.memory_index() != Register::None {
                    continue;
                }
                if base == Register::None {
                    match v {
                        Some(v) => drop(out.absolute.insert(disp, (v, size as u8))),
                        None => out.unknown += 1,
                    }
                } else if reg32(base).is_some_and(|b| this_before[b]) && (1..0x8000_0000).contains(&disp) {
                    match size {
                        4 => match v {
                            Some(v) => drop(out.this_dwords.insert(disp, v)),
                            None => out.unknown += 1,
                        },
                        // Byte values come from an immediate or the low byte of EAX..EBX only.
                        1 if imm1 || matches!(ins.op1_register(), Register::AL | Register::BL | Register::CL | Register::DL) => {
                            match v {
                                Some(v) => drop(out.this_bytes.insert(disp, v as u8)),
                                None => out.unknown += 1,
                            }
                        }
                        1 => out.unknown += 1,
                        _ => {}
                    }
                }
            }
            Mnemonic::Mov if d0.is_some() && imm1 => regs[d0.unwrap_or(0)] = Some(ins.immediate(1) as u32),
            Mnemonic::Mov if d0.is_some() && ins.op1_kind() == OpKind::Register && ins.op1_register() == Register::ECX => {
                this[d0.unwrap_or(0)] = this_before[ECX];
            }
            Mnemonic::Xor if d0.is_some() && ins.op1_kind() == OpKind::Register && ins.op1_register() == ins.op0_register() => {
                regs[d0.unwrap_or(0)] = Some(0);
            }
            Mnemonic::Inc | Mnemonic::Dec | Mnemonic::Add | Mnemonic::Sub if d0.is_some() => {
                let d = d0.unwrap_or(0);
                let step = match m {
                    Mnemonic::Inc => Some(1u32),
                    Mnemonic::Dec => Some(u32::MAX),
                    Mnemonic::Add if imm1 => Some(ins.immediate(1) as u32),
                    Mnemonic::Sub if imm1 => Some((ins.immediate(1) as u32).wrapping_neg()),
                    _ => None,
                };
                regs[d] = before[d].zip(step).map(|(v, s)| v.wrapping_add(s));
            }
            // A call may change any register but those the callee saves; ECX, EAX and EDX are lost.
            Mnemonic::Call => {
                regs = [None; 8];
                (this[0], this[ECX], this[2]) = (false, false, false);
            }
            _ => {}
        }
    }
    Some(out)
}

/// The instructions from `start` up to and including the first `ret`, within `max_len` bytes (`None` if the address
/// isn't code in the file or no `ret` comes).
pub fn until_ret(exe: &Exe, start: u32, max_len: usize) -> Option<Vec<iced_x86::Instruction>> {
    let code = exe.code(start, max_len)?;
    let mut out = Vec::new();
    for ins in Decoder::with_ip(32, code, start as u64, DecoderOptions::NONE) {
        if ins.is_invalid() {
            return None;
        }
        out.push(ins);
        if ins.mnemonic() == Mnemonic::Ret {
            return Some(out);
        }
    }
    None
}
