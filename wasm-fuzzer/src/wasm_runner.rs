pub mod socket_client;
pub mod memory_snapshot;
pub mod csv_generator;
pub mod ring_buffer;
pub mod memory_attester;
pub mod wasi_model;

use csv_generator::CsvFile;
use memory_snapshot::{MemoryState, create_memory_snapshot, analyze_memory};
use memory_attester::Model;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use ring_buffer::RingBuffer;
use crate::wasm_runner::wasi_model::WasiModel;

use super::fuzzer;

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use wasmtime::*;
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtxBuilder};
use wasmtime_wasi::p1::WasiP1Ctx;

static FUNCTION_CALL_COUNTER: AtomicUsize = AtomicUsize::new(1);
static ATTESTATION_COUNTER: AtomicUsize = AtomicUsize::new(0);
const FUEL_AMOUNT: u64 = 100_000_000_000;

pub const  SNAPSHOTS_FREQUENCY_BAD: usize = 1;
pub const  SNAPSHOTS_FREQUENCY_GOOD: usize = 1;

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum Mode {
    Normal,
    Fuzzing,
    GenerateSnapshot,
    Attestation,
    ValidationAttestation,
    ValidationAttestationBaseCase
}

pub struct Wasm {
    engine: Engine,
    mode: Mode,
    file_path: String,
    pub input_type: MemoryState 
}

impl Wasm {
    pub fn new(file_path: &str, running_mode: Mode) -> Self {
        
        let input_data: Vec<u8> = vec![0u8; 28 * 28];
        WasiModel::instance().predict(&input_data).unwrap();

        let mut config = Config::new();
        config.consume_fuel(true);

        let engine = Engine::new(&config).unwrap();
        
        return Wasm {
            engine,
            mode: running_mode,
            file_path: String::from(file_path),
            input_type: MemoryState::Good
        };
    }

    pub fn run(
        &mut self, 
        wasm_args: &[String], 
        snapshot_folder: String
    ) -> Result<(), Box<dyn std::error::Error>> {  

        let wasi_ctx: WasiP1Ctx = match self.mode {
            Mode::Fuzzing => {
                WasiCtxBuilder::new()
                    .inherit_stdio()
                    .args(wasm_args)
                    .preopened_dir(".", ".", DirPerms::all(), FilePerms::all())?
                    .build_p1()
            },
            _ => {
                WasiCtxBuilder::new()
                    .inherit_stdio()
                    .args(wasm_args)
                    .preopened_dir(".", ".", DirPerms::all(), FilePerms::all())?
                    .build_p1()
            }
        };
        
        let mut store = Store::new(&self.engine, wasi_ctx);
        store.set_fuel(FUEL_AMOUNT)?;
        let module = Module::from_file(store.engine(), self.file_path.clone())?;

        let mut linker: Linker<WasiP1Ctx> = Linker::new(&self.engine);
        wasmtime_wasi::p1::add_to_linker_sync(&mut linker, |t| t)?;

        let ml_result_buffer = Arc::new(Mutex::new(RingBuffer::new(10)));
        self.add_snapshot_function(&mut linker, &snapshot_folder, self.input_type.clone(), ml_result_buffer.clone())?;
        self.add_coverage_function(&mut linker)?;
 
        let pre_instance = linker.instantiate_pre(&module)?;
        let instance = pre_instance.instantiate(&mut store)?;

        let start_func = instance
            .get_typed_func::<(), ()>(&mut store, "_start")
            .expect("No _start function found in module");

        FUNCTION_CALL_COUNTER.store(1, Ordering::Relaxed);
        ATTESTATION_COUNTER.store(0, Ordering::Relaxed);
        match start_func.call(&mut store, ()) {
            Ok(_) => { 
                match self.mode {
                    Mode::ValidationAttestationBaseCase => CsvFile::instance().set_amount_instructions_normal(FUEL_AMOUNT - store.get_fuel().unwrap()),
                    Mode::ValidationAttestation => CsvFile::instance().set_amount_instructions_attestation(FUEL_AMOUNT - store.get_fuel().unwrap()),
                    Mode::Normal => println!("Fuel consumed: {}", FUEL_AMOUNT - store.get_fuel().unwrap()),
                    _ => ()
                }
                println!("WASM program completed successfully.");
            }

            Err(e) => {
                match self.mode {
                    Mode::Normal => println!("Fuel consumed: {}", FUEL_AMOUNT - store.get_fuel().unwrap()),
                    Mode::GenerateSnapshot => {
                        
                        let memory = instance
                            .get_memory(&mut store, "memory")
                            .ok_or("Failed to find memory export")?;

                        create_memory_snapshot(memory.data(&store), &snapshot_folder, MemoryState::Bad);
                    },
                    Mode::ValidationAttestationBaseCase => CsvFile::instance().set_amount_instructions_normal(FUEL_AMOUNT - store.get_fuel().unwrap()),
                    Mode::ValidationAttestation => {
                        let start_time = Instant::now();

                        let memory = instance
                            .get_memory(&mut store, "memory")
                            .ok_or("Failed to find memory export")?;

                        let filtered_memory: Vec<u8> = memory.data(&store)
                            .par_iter()
                            .cloned()
                            .filter(|&x| x != 0)
                            .collect();

                        CsvFile::instance().add_memory_size(filtered_memory.len());
                        let memory_state = WasiModel::instance().predict(filtered_memory.as_slice()).unwrap();
                        match memory_state {
                            MemoryState::Bad => CsvFile::instance().increase_amount_bad_results(),
                            MemoryState::Good => CsvFile::instance().increase_amount_good_results(),
                        }
                        CsvFile::instance().add_inference_time(start_time.elapsed());
                        CsvFile::instance().set_amount_instructions_attestation(FUEL_AMOUNT - store.get_fuel().unwrap());
                    },

                    _ => ()
                }

                if store.get_fuel().unwrap() == 0 {
                    return Err(Into::into("Error: Run out of fuel"));
                }

                return Err(Into::into(e));
            }
        }
    
        return Ok(())
    }

    pub fn generate_snapshots(
        &mut self, 
        wasm_args: &[String], 
        snapshot_folder: String, 
        memory_state: MemoryState
    ) -> Result<(), Box<dyn std::error::Error>> {
        
        let wasi_ctx: WasiP1Ctx = WasiCtxBuilder::new()
            .inherit_stdio()
            .args(wasm_args)
            .preopened_dir(".", ".", DirPerms::all(), FilePerms::all())?
            .build_p1();

        let mut store = Store::new(&self.engine, wasi_ctx);
        store.set_fuel(FUEL_AMOUNT)?;
        let module = Module::from_file(store.engine(), self.file_path.clone())?;
        
        let mut linker: Linker<WasiP1Ctx> = Linker::new(&self.engine);
        wasmtime_wasi::p1::add_to_linker_sync(&mut linker, |t| t)?;

        let ml_result_buffer = Arc::new(Mutex::new(RingBuffer::new(10)));
        self.add_snapshot_function(&mut linker, &snapshot_folder, memory_state.clone(), ml_result_buffer.clone())?;  
        self.add_coverage_function(&mut linker)?;

        let pre_instance = linker.instantiate_pre(&module)?;
        let instance = pre_instance.instantiate(&mut store)?;

        let start_func = instance
            .get_typed_func::<(), ()>(&mut store, "_start")
            .expect("No _start function found in module");

        println!("Starting wasm program");
        FUNCTION_CALL_COUNTER.store(1, Ordering::Relaxed);
        match start_func.call(&mut store, ()) {
            Ok(_) => { 
                
                match memory_state {
                    MemoryState::Bad => {
                        let memory = instance
                            .get_memory(&mut store, "memory")
                            .ok_or("Failed to find memory export")?;
                    
                        create_memory_snapshot(memory.data(&store), &snapshot_folder, MemoryState::Bad);
                    },
                    MemoryState::Good => {
                        let memory = instance
                            .get_memory(&mut store, "memory")
                            .ok_or("Failed to find memory export")?;
                
                        create_memory_snapshot(memory.data(&store), &snapshot_folder, MemoryState::Good);
                    }
                }
                println!("Fuel consumed: {}", FUEL_AMOUNT - store.get_fuel().unwrap())
            }
            Err(_e) => {
                
                match memory_state {
                    MemoryState::Bad => {
                        let memory = instance
                            .get_memory(&mut store, "memory")
                            .ok_or("Failed to find memory export")?;
                    
                        create_memory_snapshot(memory.data(&store), &snapshot_folder, MemoryState::Bad);
                    },
                    MemoryState::Good => {
                        let memory = instance
                            .get_memory(&mut store, "memory")
                            .ok_or("Failed to find memory export")?;
                
                        create_memory_snapshot(memory.data(&store), &snapshot_folder, MemoryState::Good);
                    }
                }
                println!("Fuel consumed: {}", FUEL_AMOUNT - store.get_fuel().unwrap())         
            }
        }
    
        return Ok(())
    }

    fn add_snapshot_function(
        &mut self, 
        linker: &mut Linker<WasiP1Ctx>, 
        snapshot_folder: &String, 
        memory_state: MemoryState,
        ml_result_buffer: Arc<Mutex<RingBuffer<MemoryState>>>
    ) -> Result<(), Box<dyn std::error::Error>> {

        let running_mode = self.mode.clone();
        let ml_result_buffer = ml_result_buffer.clone();

        linker.func_wrap("env", "create_memory_snapshot", {          
            let memory_snapshot_folder = snapshot_folder.clone();

            move |mut caller: Caller<'_, WasiP1Ctx>| {
                if running_mode == Mode::Fuzzing || running_mode == Mode::ValidationAttestationBaseCase{
                    return Ok(());
                }

                let function_call_counter = FUNCTION_CALL_COUNTER.fetch_add(1, Ordering::Relaxed);
                if memory_state == MemoryState::Bad && function_call_counter % SNAPSHOTS_FREQUENCY_BAD != 0 {
                    return Ok(());
                }
                if memory_state == MemoryState::Good && function_call_counter % SNAPSHOTS_FREQUENCY_GOOD != 0 {
                    return Ok(());
                }

                let memory = caller
                    .get_export("memory")
                    .and_then(|e| e.into_memory())
                    .ok_or_else(|| "Failed to find memory export")
                    .unwrap();

                if running_mode == Mode::Normal{
                    create_memory_snapshot(memory.data(&caller), &memory_snapshot_folder, MemoryState::Good);
                }

                if running_mode == Mode::GenerateSnapshot {
                    create_memory_snapshot(memory.data(&caller), &memory_snapshot_folder, memory_state.clone());
                }

                if running_mode == Mode::Attestation {
                    let memory_state = analyze_memory(memory.data(&caller));
                    let mut buffer = ml_result_buffer.lock().unwrap();
                    buffer.push(memory_state);

                    // Check if the majority of the results are bad
                    let amount_bad_results = buffer
                        .get_stack()
                        .iter()
                        .filter(|&state| state == &MemoryState::Bad)
                        .count();

                    if amount_bad_results >= buffer.get_max_size() / 2 + 1{
                        println!("Bad state detected");
                    }
                }

                if running_mode == Mode::ValidationAttestation {
                    ATTESTATION_COUNTER.fetch_add(1, Ordering::Relaxed);
                    let start_time = Instant::now();
                    // let memory_state = analyze_memory(memory.data(&caller));
                    let filtered_memory: Vec<u8> = memory.data(&caller)
                        .par_iter()
                        .cloned()
                        .filter(|&x| x != 0)
                        .collect();

                    CsvFile::instance().add_memory_size(filtered_memory.len());
                    let memory_state = WasiModel::instance().predict(filtered_memory.as_slice()).unwrap();

                    let mut buffer = ml_result_buffer.lock().unwrap();
                    buffer.push(memory_state.clone());
                    
                    match memory_state {
                        MemoryState::Bad => CsvFile::instance().increase_amount_bad_results(),
                        MemoryState::Good => CsvFile::instance().increase_amount_good_results(),
                    }

                    // Check if the majority of the results are bad
                    let amount_bad_results = buffer
                        .get_stack()
                        .iter()
                        .filter(|&state| state == &MemoryState::Bad)
                        .count();

                    if amount_bad_results >= buffer.get_max_size() / 2 + 1{
                        CsvFile::instance().set_final_attestation_verdict(MemoryState::Bad);
                        let verdict_counter = ATTESTATION_COUNTER.load(Ordering::Relaxed);
                        CsvFile::instance().set_final_verdict_time(verdict_counter.try_into().expect("Failed to convert FUNCTION_CALL_COUNTER to u32"));
                    }

                    CsvFile::instance().add_inference_time(start_time.elapsed());

                    if CsvFile::instance().get_memory_sizes_length() >= 1000 {
                        CsvFile::instance().add_csv_entry();
                        std::process::exit(0);
                    }
                }

                return Ok(());
            }
        })?;

        return Ok(());
    }

    fn add_coverage_function(
        &mut self, 
        linker: &mut Linker<WasiP1Ctx>
    )-> Result<(), Box<dyn std::error::Error>> {
        linker.func_wrap("env", "update_coverage", {          
            move |mut _caller: Caller<'_, WasiP1Ctx>, ptr: i32| {
                
                if ptr >= 0 {
                    let idx = ptr as usize;
                    if idx < fuzzer::SIGNALS_LEN {
                        fuzzer::signals_set(idx);
                    } 
                    else {
                        eprintln!("Coverage index out of bounds: {}", idx);
                    }
                }
                return Ok(())
            }
        })?;

        return Ok(());
    }
}