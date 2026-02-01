use libafl::{
    corpus::{CachedOnDiskCorpus, OnDiskCorpus},
    events::SimpleEventManager,
    executors::{inprocess::InProcessExecutor, ExitKind},
    feedbacks::{CrashFeedback, MaxMapFeedback, TimeoutFeedback, FastOrFeedback},
    fuzzer::{Fuzzer, StdFuzzer},
    inputs::{BytesInput, HasTargetBytes},
    monitors::SimpleMonitor,
    mutators::{havoc_mutations, scheduled::StdScheduledMutator},
    observers::StdMapObserver,
    schedulers::StdScheduler,
    stages::mutational::StdMutationalStage,
    state::StdState,
};
use libafl_bolts::{current_nanos, rands::StdRand, tuples::tuple_list, AsSlice};

use super::wasm_runner::{Mode, Wasm};
use super::{FUZZER_INPUTS_FOLDER, INSTRUMENT_FUZZING_PREFIX};
use super::make_wasm_args;

use std::{path::PathBuf, time::Duration};
use std::io::Write;
use std::fs::File;
use sha2::{Sha256, Digest};

static FUZZER_CRASHES_FOLDER: &str = "fuzzer-output/crashes";
static FUZZER_CORPUS_FOLDER: &str = "fuzzer-output/corpus";
static ORIGINAL_CORPUS_FOLER: &str = "wasm_files/corpus";
static WASM_FILES_FOLDER: &str = "wasm_files/asan";

pub static SIGNALS_LEN: usize = 8_192; 
static mut SIGNALS: [u8; SIGNALS_LEN] = [0; SIGNALS_LEN];

pub fn signals_set(idx: usize) {
    unsafe { 
        SIGNALS[idx] = 1;
    };
}

fn reset_signals() {
  unsafe {
        #[allow(static_mut_refs)]
        SIGNALS.fill(0);
    }
}

fn count_signals() -> usize{
    unsafe {
        #[allow(static_mut_refs)]
        return SIGNALS.iter().filter(|&&x| x == 1).count();
    }
}

fn create_fuzzer_input_file(file_name: &str, input: &[u8]) {
    let mut file = File::create(file_name).unwrap();
    file.write_all(input).unwrap();
}

pub fn run(wasm_file: &String, read_input_from_file: bool) {
    let input_file = "fuzzer-output/input.abc";
    let output_file_ending = ".abc";
    let wasm_file_path = format!("{}/{}{}.wasm", WASM_FILES_FOLDER, INSTRUMENT_FUZZING_PREFIX, wasm_file);

    let mut harness = |input: &BytesInput| {
        reset_signals();
        let mut wasm_program = Wasm::new(&wasm_file_path, Mode::Fuzzing);
        let target = input.target_bytes();
        let buffer = target.as_slice();

        let input_arg = if read_input_from_file {
            create_fuzzer_input_file(input_file, buffer);
            input_file.to_string()
        } else {
            String::from_utf8_lossy(buffer).into_owned()
        };

        let wasm_args = make_wasm_args(&wasm_file_path, &input_arg, true);

        let mut hasher = Sha256::new();
        hasher.update(buffer);
        let input_hash = hasher.finalize_reset();

        match wasm_program.run(&wasm_args, String::from(FUZZER_INPUTS_FOLDER)) {
            Ok(_) => {
                let file_name = format!("{}/good/{}{}", FUZZER_INPUTS_FOLDER, hex::encode(input_hash), output_file_ending);
                create_fuzzer_input_file(&file_name, buffer);
                return ExitKind::Ok;
            } 
            
            Err(e) => {
                let error = e.to_string();
                
                if error.contains("fuel") {
                    let file_name = format!("{}/bad/{}{}", FUZZER_INPUTS_FOLDER, hex::encode(input_hash), output_file_ending);
                    create_fuzzer_input_file(&file_name, buffer);
                    println!("Run out of fuel");

                    return ExitKind::Timeout;
                }

                if error.contains("!_Exit") || error.contains("!exit") {
                    let file_name1 = format!("{}/good/{}{}", FUZZER_INPUTS_FOLDER, hex::encode(input_hash), output_file_ending);
                    create_fuzzer_input_file(&file_name1, buffer);

                    return ExitKind::Ok;
                }

                let file_name = format!("{}/bad/{}{}", FUZZER_INPUTS_FOLDER, hex::encode(input_hash), output_file_ending);
                create_fuzzer_input_file(&file_name, buffer);
                println!("Program crash");
                return ExitKind::Crash;
            }
        }
    };

    let observer = unsafe { StdMapObserver::new("signals", &mut SIGNALS[..]) };
    let mut feedback = MaxMapFeedback::new(&observer);
    let corpus = CachedOnDiskCorpus::<BytesInput>::new(FUZZER_CORPUS_FOLDER, 30)
        .expect("Failed to create corpus");

    let crash_objective = CrashFeedback::new();
    let timeout_objective = TimeoutFeedback::new();
    let mut objective = FastOrFeedback::new(crash_objective, timeout_objective);

    let mut state = StdState::new(
        StdRand::with_seed(current_nanos()),
        corpus,
        OnDiskCorpus::new(PathBuf::from(FUZZER_CRASHES_FOLDER)).unwrap(),
        &mut feedback,
        &mut objective,
    ).unwrap();

    let monitor = SimpleMonitor::new(|s| 
        println!("{s}, edges: {}/{}", count_signals(), SIGNALS_LEN)
    );

    let mut event_manager = SimpleEventManager::new(monitor);
    let scheduler = StdScheduler::new();
    let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);

    let mut executor = InProcessExecutor::with_timeout(
        &mut harness,
        tuple_list!(observer),
        &mut fuzzer,
        &mut state,
        &mut event_manager,
        Duration::from_secs(60)
    ).expect("Failed to create the InProcessExecutor");

    state.load_initial_inputs(
        &mut fuzzer, 
        &mut executor, 
        &mut event_manager, 
        &[PathBuf::from(format!("{}/{}/", ORIGINAL_CORPUS_FOLER, wasm_file))]
    ).unwrap();

    let mutator = StdScheduledMutator::new(havoc_mutations());
    let mut stages = tuple_list!(StdMutationalStage::new(mutator));

    fuzzer
        .fuzz_loop(&mut stages, &mut executor, &mut state, &mut event_manager)
        .expect("Error in the fuzzing loop");
}