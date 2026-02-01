use wasmtime::*;
use wasmtime_wasi::WasiCtxBuilder;
use wasmtime_wasi::p1::WasiP1Ctx;
use wasmtime_wasi_nn::witx::WasiNnCtx;
use wasmtime_wasi_nn::{InMemoryRegistry, Backend};
use once_cell::sync::Lazy;
use std::error::Error;
use std::sync::Mutex;
use anyhow::Context;
use crate::wasm_runner::memory_snapshot::MemoryState;

const WASM_PAGE_SIZE: usize = 65536;
const DEFAULT_INPUT_OFFSET: usize = 1024;
static INFERENCE_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

struct ModelData {
    engine: Engine,
    module: Module,
    linker: Linker<Ctx>,
}

static WASI_MODEL: Lazy<ModelData> = Lazy::new(|| {
    ModelData::new().expect("Failed to load wasi-nn module")
});

struct Ctx {
    wasi: WasiP1Ctx,
    wasi_nn: WasiNnCtx,
}

impl ModelData {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut config = Config::new();
        config.wasm_backtrace_details(WasmBacktraceDetails::Enable);
        config.max_wasm_stack(2 * 1024 * 1024);

        let engine = Engine::new(&config)?;
        let module = Module::from_file(&engine, "wasm_files/models/resnet.wasm")?;
        
        let mut linker: Linker<Ctx> = Linker::new(&engine);
        wasmtime_wasi::p1::add_to_linker_sync(&mut linker, |t| &mut t.wasi)?;
        wasmtime_wasi_nn::witx::add_to_linker(&mut linker, |t| &mut t.wasi_nn)?;
        
        Ok(Self {
            engine,
            module,
            linker,
        })
    }
}

struct InitializedModel {
    store: Store<Ctx>,
    instance: Instance,
    run_inference_func: TypedFunc<(i32, i32), f32>,
    memory: Memory,
}

static INITIALIZED_MODEL: Lazy<Mutex<Option<InitializedModel>>> = Lazy::new(|| {
    Mutex::new(None)
});

pub struct WasiModel;

impl WasiModel {
    pub fn instance() -> Self {
        
        let _ = &*WASI_MODEL;
        WasiModel
    }
    
    fn create_context() -> Result<Ctx, Box<dyn std::error::Error>> {
        let wasi_ctx = WasiCtxBuilder::new()
            .inherit_stdio()
            .preopened_dir(
                "./wasm_files/models/wasi-nn",
                "wasi-nn",
                wasmtime_wasi::DirPerms::all(),
                wasmtime_wasi::FilePerms::all(),
            )?
            .build_p1();
        
        let registry = InMemoryRegistry::new().into();
        let backends = vec![Backend::from(
            wasmtime_wasi_nn::backend::openvino::OpenvinoBackend::default(),
        )];
        let wasi_nn_ctx = WasiNnCtx::new(backends, registry);
        
        Ok(Ctx {
            wasi: wasi_ctx,
            wasi_nn: wasi_nn_ctx,
        })
    }
    
pub fn predict(&self, byte_array: &[u8]) -> Result<MemoryState, Box<dyn Error>> {
    
    let _inference_guard = INFERENCE_LOCK.lock().unwrap();

    let mut model_lock = INITIALIZED_MODEL.lock().unwrap();
    if model_lock.is_none() {
        println!("Initializing WASI-NN model for the first time...");

        let ctx = Self::create_context()?;
        let mut store = Store::new(&WASI_MODEL.engine, ctx);
        let instance = WASI_MODEL.linker.instantiate(&mut store, &WASI_MODEL.module)?;

        let init_model = instance.get_typed_func::<(), u32>(&mut store, "init_model")?;
        let run_inference = instance.get_typed_func::<(i32, i32), f32>(&mut store, "run_inference")?;
        let memory = instance.get_memory(&mut store, "memory")
            .ok_or_else(|| anyhow::anyhow!("failed to find wasm memory"))?;

        let graph_id = init_model.call(&mut store, ())?;
        println!("Model initialized with graph ID: {}", graph_id);

        *model_lock = Some(InitializedModel {
            store,
            instance,
            run_inference_func: run_inference,
            memory,
        });
    }

    let model = model_lock.as_mut().unwrap();

    let input_ptr = DEFAULT_INPUT_OFFSET;
    let required_end = input_ptr
        .checked_add(byte_array.len())
        .ok_or_else(|| anyhow::anyhow!("pointer overflow for input buffer"))?;

    let current_size = model.memory.data_size(&model.store);
    if required_end > current_size {
        let extra = required_end - current_size;
        let pages_needed = (extra + WASM_PAGE_SIZE - 1) / WASM_PAGE_SIZE;
        model.memory.grow(&mut model.store, pages_needed as u64)
            .context("failed to grow wasm memory for input data")?;
    }

    model.memory.data_mut(&mut model.store)[input_ptr..required_end]
        .copy_from_slice(byte_array);

    let result = model.run_inference_func.call(
        &mut model.store,
        (input_ptr as i32, byte_array.len() as i32),
    )?;

    if result > 0.5 {
        Ok(MemoryState::Bad)
    } else {
        Ok(MemoryState::Good)
    }
}

}