use walrus::{
    self, FunctionId, ImportId, InstrLocId, LocalFunction, ir::{Instr, InstrSeqId, Value}
};
use std::borrow::BorrowMut;
use std::collections::HashSet;

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum InstrumentMode {
    LocalFunction,
    ImportFunction,
    MemoryInstruction,
    Fuzzing
}

impl InstrumentMode {
    pub const VALUES: [InstrumentMode; 4] = [
        InstrumentMode::LocalFunction,
        InstrumentMode::ImportFunction,
        InstrumentMode::MemoryInstruction,
        InstrumentMode::Fuzzing
    ];
}

pub fn instrument_wasm_file(in_path: &str, out_path: &str, mode: InstrumentMode) {
    let mut module = walrus::Module::from_file(in_path).unwrap();

    let func_type = match mode {
        InstrumentMode::LocalFunction => module.types.add(&[], &[]),
        InstrumentMode::ImportFunction => module.types.add(&[], &[]),
        InstrumentMode::MemoryInstruction => module.types.add(&[], &[]),
        InstrumentMode::Fuzzing => module.types.add(&[walrus::ValType::I32], &[]),
    };

    let extern_function = module.add_import_func(
        "env", 
        match mode {
            InstrumentMode::LocalFunction => "create_memory_snapshot",
            InstrumentMode::ImportFunction => "create_memory_snapshot",
            InstrumentMode::MemoryInstruction => "create_memory_snapshot",
            InstrumentMode::Fuzzing => "update_coverage",
        },
        func_type
    );

    let mut instrument_counter:i32 = 0;
    
    let imported_function_ids: Vec<FunctionId> = if mode == InstrumentMode::ImportFunction {
        module.funcs
            .iter()
            .filter(|func| {
                matches!(func.kind, walrus::FunctionKind::Import(_)) && func.id() != extern_function.0
            })
            .map(|func| func.id())
            .collect()
    } else {
        Vec::new()
    };

    for func in module.funcs.iter_mut() {
        let is_local = matches!(func.kind, walrus::FunctionKind::Local(_));

        if is_local {
            let mut local_function = func.kind.borrow_mut().unwrap_local_mut();
            let function_sequence = local_function.entry_block();

            if mode == InstrumentMode::LocalFunction {
                
                instrument_counter += 1;
                local_function
                    .builder_mut()
                    .instr_seq(function_sequence)
                    .call(extern_function.0);
            }
            if mode == InstrumentMode::ImportFunction {
                instrument_import_calls(&mut local_function, &mut instrument_counter, extern_function.clone(), &imported_function_ids);
            }
            if mode == InstrumentMode::MemoryInstruction {
                instrument_memory_instructions(&mut local_function, &mut instrument_counter, extern_function.clone());
            }
            if mode == InstrumentMode::Fuzzing {
                instrument_branches(&mut local_function, &mut instrument_counter, extern_function.clone());

                instrument_counter += 1;
                local_function
                    .builder_mut()
                    .instr_seq(function_sequence)
                    .i32_const(instrument_counter)
                    .call(extern_function.0);
            }
        }
    }
    println!("Instrumented {} functions/instructions for {:?}", instrument_counter, mode);
    module.emit_wasm_file(out_path).unwrap();
}

fn instrument_import_calls(
    function: &mut LocalFunction,
    instrument_counter: &mut i32,
    extern_function: (FunctionId, ImportId),
    imported_function_ids: &[FunctionId],
) {
    let imported: HashSet<FunctionId> = imported_function_ids.iter().cloned().collect();

    let mut call_sites: Vec<(InstrSeqId, usize)> = Vec::new();

    fn visit_seq(
        seq_id: InstrSeqId,
        func: &LocalFunction,
        imported: &HashSet<FunctionId>,
        out: &mut Vec<(InstrSeqId, usize)>
    ) {
        let seq = func.block(seq_id);
        let instrs = &seq.instrs;

        for (idx, (instr, _loc)) in instrs.iter().enumerate() {
            match instr {
                Instr::Call(call) => {
                    if imported.contains(&call.func) {
                        out.push((seq_id, idx));
                    }
                }

                Instr::Block(b) => {
                    visit_seq(b.seq, func, imported, out);
                }

                Instr::Loop(l) => {
                    visit_seq(l.seq, func, imported, out);
                }

                Instr::IfElse(e) => {
                    visit_seq(e.consequent, func, imported, out);
                    visit_seq(e.alternative, func, imported, out);
                }

                _ => {}
            }
        }
    }

    visit_seq(function.entry_block(), function, &imported, &mut call_sites);

    use std::collections::HashMap;
    let mut by_seq: HashMap<InstrSeqId, Vec<usize>> = HashMap::new();

    for (seq, idx) in call_sites {
        by_seq.entry(seq).or_default().push(idx);
    }

    for (seq, mut idxs) in by_seq {
        idxs.sort_unstable_by(|a, b| b.cmp(a));

        for idx in idxs {
            *instrument_counter += 1;

            function
                .builder_mut()
                .instr_seq(seq)
                .call_at(idx + 1, extern_function.0);
        }
    }
}

fn instrument_branches(
    function: &mut LocalFunction, 
    instrument_counter: &mut i32,
    extern_function: (FunctionId, ImportId)
) {
    
    let block_ids: Vec<(Instr, InstrLocId)> = {
        let builder = function.builder_mut();
        let function_body = builder.func_body();

        function_body
            .instrs()
            .iter()
            .filter(|instruction| instruction.0.is_block())
            .cloned()
            .collect()
    };

    for block_id in block_ids {
        let block_seq = block_id.0.unwrap_block().seq;

        let indicies: Vec<usize> = {
            let instruction_sequence = &function.block_mut(block_seq).instrs;
            instruction_sequence
                .iter()
                .enumerate()
                .filter_map(|(index, instruction)| {
                    if instruction.0.is_br() || instruction.0.is_br_if() || instruction.0.is_if_else() {
                        Some(index)
                    } 
                    else {
                        None
                    }
                })
                .collect()
        };

        for index in indicies.iter() {
            *instrument_counter += 1;
            function
                .builder_mut()
                .instr_seq(block_seq)
                .const_at(index + 1, Value::I32(*instrument_counter))
                .call_at(index + 2, extern_function.0);
        }
    }
}

fn instrument_memory_instructions(
    function: &mut LocalFunction, 
    instrument_counter: &mut i32,
    extern_function: (FunctionId, ImportId)
) {
    let contains_memory_instruction: bool = {
        let builder = function.builder_mut();
        let function_body = builder.func_body();

        function_body
            .instrs()
            .iter()
            .any(|instruction|
                instruction.0.is_store() ||
                instruction.0.is_memory_copy() ||
                instruction.0.is_memory_fill() ||
                instruction.0.is_memory_init() ||
                instruction.0.is_memory_grow()
            )
    };

    if contains_memory_instruction {
        let function_sequence = function.entry_block();
        function
            .builder_mut()
            .instr_seq(function_sequence)
            .call(extern_function.0);

        *instrument_counter += 1;
    }

    let block_ids: Vec<(Instr, InstrLocId)> = {
        let builder = function.builder_mut();
        let function_body = builder.func_body();

        function_body
            .instrs()
            .iter()
            .filter(|instruction| instruction.0.is_block())
            .cloned()
            .collect()
    };

    for block_id in block_ids {
        let block_seq = block_id.0.unwrap_block().seq;

        let contains_memory_instruction: bool = {
            let instruction_sequence = &function.block_mut(block_seq).instrs;
            
            instruction_sequence
                .iter()
                .any(|instruction|
                    instruction.0.is_store() ||
                    instruction.0.is_memory_copy() ||
                    instruction.0.is_memory_fill() ||
                    instruction.0.is_memory_init() ||
                    instruction.0.is_memory_grow()
                )
        };

        if contains_memory_instruction {
            *instrument_counter += 1;
            function
                .builder_mut()
                .instr_seq(block_seq)
                .call(extern_function.0);
        }
    }
}