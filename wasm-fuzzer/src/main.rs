mod console_messages;
mod fuzzer;
mod instrumenter;

mod wasm_runner;
use wasm_runner::{memory_snapshot::{self, MemoryState}, Mode, Wasm};
use wasm_runner::csv_generator::CsvFile;
use instrumenter::InstrumentMode;

use std::env;
use std::time::Instant;
use std::fs::{self};
use std::path::{Path, PathBuf};

pub static FUZZER_INPUTS_FOLDER: &str = "fuzzer-output/inputs";
static SNAPSHOTS_FOLDER: &str = "fuzzer-output/snapshots";
static IMAGE_FOLDER: &str = "fuzzer-output/images";
static VALIDATION_FOLDER: &str = "wasm_files/validation";

static WASM_FILES_FOLDER: &str = "wasm_files";

static INSTRUMENT_FUZZING_PREFIX: &str = "fuzzing-";
static INSTRUMENT_ATTESTATION_LOCAL_PREFIX: &str = "instr-local-";
static INSTRUMENT_ATTESTATION_IMPORT_PREFIX: &str = "instr-import-";
static INSTRUMENT_ATTESTATION_MEMORY_PREFIX: &str = "instr-memory-";

static WASM_ARGS: [&str; 2] = [
    "WASM_FILE",
    "INPUT_FILE"
];

fn get_wasm_file_name(path: &str) -> Option<&str> {
    return Path::new(path)
        .file_name()  
        .and_then(|name| name.to_str())  
        .filter(|name| name.contains(".wasm"));  
}

pub fn make_wasm_args(wasm_file: &String, input_file: &String, read_input_from_file: bool) -> Vec<String> {
    WASM_ARGS
        .iter()
        .map(|&arg| match arg {
            "WASM_FILE" => wasm_file.clone(),
            "INPUT_FILE" => {
                if read_input_from_file {
                    input_file.clone()
                }
                else {
                    let file_content = fs::read(input_file).expect(format!("Failed to read file: {}", input_file.as_str()).as_str());
                    String::from_utf8_lossy(&file_content).into_owned()
                }
            },
            _ => arg.to_string(),
        })
        .collect()
}

fn instrument_wasm_file(in_path: &String, out_folder: &String, mode: InstrumentMode) {
    let original_file_name: &str = get_wasm_file_name(in_path).unwrap_or_else(|| {
        println!("No wasm file found");
        std::process::exit(0);
    }); 

    let out_file = match mode {
        InstrumentMode::LocalFunction => out_folder.clone() + "/" + INSTRUMENT_ATTESTATION_LOCAL_PREFIX + original_file_name,
        InstrumentMode::ImportFunction => out_folder.clone() + "/" + INSTRUMENT_ATTESTATION_IMPORT_PREFIX + original_file_name,
        InstrumentMode::MemoryInstruction => out_folder.clone() + "/" + INSTRUMENT_ATTESTATION_MEMORY_PREFIX + original_file_name,
        InstrumentMode::Fuzzing => out_folder.clone() + "/" + INSTRUMENT_FUZZING_PREFIX + original_file_name,
    };   
    
    std::fs::create_dir_all(out_folder).unwrap();
    instrumenter::instrument_wasm_file(in_path, &out_file, mode);
    std::process::exit(0);
}

fn instrument_wasm_files(wasm_file: &String) {
    for value in InstrumentMode::VALUES.iter() {
        let input_wasm_file: String = match value {
            InstrumentMode::Fuzzing => format!("{}/asan/{}.wasm", WASM_FILES_FOLDER, wasm_file),
            _ => format!("{}/no-asan/{}.wasm", WASM_FILES_FOLDER, wasm_file)
        };
        let output_file: String = match value {
            InstrumentMode::Fuzzing => format!("{}/asan/{}{}.wasm", WASM_FILES_FOLDER, INSTRUMENT_FUZZING_PREFIX, wasm_file),
            InstrumentMode::LocalFunction => format!("{}/no-asan/{}{}.wasm", WASM_FILES_FOLDER, INSTRUMENT_ATTESTATION_LOCAL_PREFIX, wasm_file),
            InstrumentMode::ImportFunction => format!("{}/no-asan/{}{}.wasm", WASM_FILES_FOLDER, INSTRUMENT_ATTESTATION_IMPORT_PREFIX, wasm_file),
            InstrumentMode::MemoryInstruction => format!("{}/no-asan/{}{}.wasm", WASM_FILES_FOLDER, INSTRUMENT_ATTESTATION_MEMORY_PREFIX, wasm_file),
        };

        instrumenter::instrument_wasm_file(&input_wasm_file, &output_file, value.clone());
    }
}

fn compare_memory_snapshots(file1: &String, file2: &String) {
    let are_equal = wasm_runner::memory_snapshot::compare_memory_snapshots(file1, file2);
    println!("Are the memory snapshots identical? {}", are_equal);
    std::process::exit(0);
}

fn run_wasm_file(wasm_file: &String, input_file: &String, read_input_from_file: bool) {
    let output_folder = String::from("fuzzer-output");
    wasm_runner::memory_snapshot::create_snapshot_folders(&output_folder);

    let mut wasm_program = Wasm::new(wasm_file.as_str(), Mode::Normal);    
    let wasm_args: Vec<String> = make_wasm_args(wasm_file, input_file, read_input_from_file);

    match wasm_program.run(&wasm_args, output_folder.clone()) {
        Ok(_) => {} 
        Err(e) => {
            println!("{}", e.to_string());
        }
    }
}

fn collect_memory_snapshots(wasm_file: &str, input_folder: &str, read_input_from_file: bool) {
    let mut wasm_program = Wasm::new(wasm_file, Mode::GenerateSnapshot);
    let mut counter = 0;
    for entry in fs::read_dir(input_folder).unwrap() {
        let path = entry.unwrap().path();

        if path.is_dir() {
            collect_memory_snapshots(wasm_file, path.to_str().unwrap(), read_input_from_file);
        }

        if path.is_file() {
            counter += 1;
            print!("Processing File: {}\nFile Name: {}\n", counter, path.to_str().unwrap());
            
            let last_folder = path
                .parent().unwrap()
                .file_name().unwrap();
            let wasm_args = make_wasm_args(
                &String::from(wasm_file), 
                &String::from(path.to_str().unwrap()), 
                read_input_from_file
            );

            match last_folder.to_str().unwrap() {
                "good" => {
                    wasm_program.generate_snapshots(&wasm_args, String::from(SNAPSHOTS_FOLDER), MemoryState::Good).unwrap()
                },
                "bad" => {
                    wasm_program.generate_snapshots(&wasm_args, String::from(SNAPSHOTS_FOLDER), MemoryState::Bad).unwrap()
                }
                _ => return
            }
        }
    }
}

fn convert_rle_to_png(input_folder: &str) {
    let mut counter = 0;
    for entry in fs::read_dir(input_folder).unwrap() {
        let path = entry.unwrap().path();

        if path.is_dir() {
            convert_rle_to_png(path.to_str().unwrap());
        }

        if path.is_file() {
            counter += 1;
            println!("Coverting File: {counter}");

            let last_folder = path
                .parent().unwrap()
                .file_name().unwrap();

            let file_name = path.file_stem().unwrap();

            let mut output_file = PathBuf::from(IMAGE_FOLDER);
            output_file.push(last_folder);
            
            output_file.push(file_name);
            output_file.set_extension("png");
            memory_snapshot::rle_to_png(path.to_str().unwrap(), output_file.to_str().unwrap());
        }
    }
}

fn attest_wasm_file(wasm_file: &str, input_file: &str) {
    let mut wasm_program = Wasm::new(wasm_file, Mode::Attestation);
    let wasm_args = [
        String::from(wasm_file), 
        String::from(input_file),
        
    ];
    
    wasm_program.run(&wasm_args, "".to_string()).unwrap();
}

fn validate_ml_model(wasm_file_name: &str, instrument_type: &str, input_folder: &str, read_input_from_file: bool) {
    for entry in fs::read_dir(input_folder).unwrap() {
        let path = entry.unwrap().path();

        if path.is_dir() {
            validate_ml_model(wasm_file_name, instrument_type, path.to_str().unwrap(), read_input_from_file);
        }

        if path.is_file() {
            
            let last_folder = path
                .parent().unwrap()
                .file_name().unwrap();
            let input_file = String::from(path.to_str().unwrap()); 
            let mut wasm_args = make_wasm_args(
                &String::from(wasm_file_name), 
                &String::from(path.to_str().unwrap()), 
                read_input_from_file
            );

            CsvFile::instance().clear();
            CsvFile::instance().set_input_file_name(input_file.split("/").last().unwrap().to_string());
            match last_folder.to_str().unwrap() {
                "good" => {
                    CsvFile::instance().set_input_type(MemoryState::Good);
                    CsvFile::instance().set_snapshot_frequency(wasm_runner::SNAPSHOTS_FREQUENCY_GOOD);
                },
                "bad" => {
                    CsvFile::instance().set_input_type(MemoryState::Bad);
                    CsvFile::instance().set_snapshot_frequency(wasm_runner::SNAPSHOTS_FREQUENCY_BAD);
                }
                _ => panic!("The input folder has the wrong structure. Do not recognize folder: {}", last_folder.to_str().unwrap())
            }

            let attestation_wasm_file = format!("{}/no-asan/{}{}.wasm", WASM_FILES_FOLDER, instrument_type, wasm_file_name);
            wasm_args[0] = attestation_wasm_file.clone();
            println!("Running file: {}", attestation_wasm_file);
            
            let mut normal_wasm_program = Wasm::new(&attestation_wasm_file, Mode::ValidationAttestationBaseCase);
            normal_wasm_program.input_type = match last_folder.to_str().unwrap() {
                "good" => MemoryState::Good,
                "bad" => MemoryState::Bad,
                _ => panic!("The input folder has the wrong structure. Do not recognize folder: {}", last_folder.to_str().unwrap())
            };

            let normal_wasm_time = Instant::now();
            match normal_wasm_program.run(&wasm_args, SNAPSHOTS_FOLDER.to_string()) {
                Ok(_) => {
                    CsvFile::instance().set_runtime_normal(normal_wasm_time.elapsed());
                    CsvFile::instance().set_final_normal_verdict(MemoryState::Good);
                } 
                Err(e) => {
                    CsvFile::instance().set_runtime_normal(normal_wasm_time.elapsed());
                    let error_string = e.to_string();

                    if error_string.contains("!_Exit") || error_string.contains("!exit") {
                        CsvFile::instance().set_final_normal_verdict(MemoryState::Good);
                    }
                    else {
                        CsvFile::instance().set_final_normal_verdict(MemoryState::Bad);
                    }
                }        
            }

            let mut attestation_wasm_program = Wasm::new(&attestation_wasm_file, Mode::ValidationAttestation);
            attestation_wasm_program.input_type = match last_folder.to_str().unwrap() {
                "good" => MemoryState::Good,
                "bad" => MemoryState::Bad,
                _ => panic!("The input folder has the wrong structure. Do not recognize folder: {}", last_folder.to_str().unwrap())
            };

            let attestation_wasm_time = Instant::now();
            match attestation_wasm_program.run(&wasm_args, SNAPSHOTS_FOLDER.to_string()) {
                Ok(_) => {
                    CsvFile::instance().set_runtime_attestation(attestation_wasm_time.elapsed());
                } 
                Err(_) => {
                    CsvFile::instance().set_runtime_attestation(attestation_wasm_time.elapsed());
                }        
            }

            CsvFile::instance().add_csv_entry();
        }
    }
}

fn instrument_binaries(binary_name: &String) {    
    for value in InstrumentMode::VALUES.iter() {
        if *value == InstrumentMode::Fuzzing {
            continue;
        }

        let input_wasm_file: String = format!("{}/binaries/{}/{}.wasm", WASM_FILES_FOLDER, binary_name, binary_name);
        let output_file: String = match value {
            InstrumentMode::LocalFunction => format!("{}/binaries/{}/{}{}.wasm", WASM_FILES_FOLDER, binary_name, INSTRUMENT_ATTESTATION_LOCAL_PREFIX, binary_name),
            InstrumentMode::ImportFunction => format!("{}/binaries/{}/{}{}.wasm", WASM_FILES_FOLDER, binary_name, INSTRUMENT_ATTESTATION_IMPORT_PREFIX, binary_name),
            InstrumentMode::MemoryInstruction => format!("{}/binaries/{}/{}{}.wasm", WASM_FILES_FOLDER, binary_name, INSTRUMENT_ATTESTATION_MEMORY_PREFIX, binary_name),
            _ => continue
        };

        instrumenter::instrument_wasm_file(&input_wasm_file, &output_file, value.clone());
    }

    let text_file = std::path::PathBuf::from(format!(
        "../memory-analyzer/csv-files/binaries/{}/instr-info.txt",
        binary_name
    ));

    fs::create_dir_all(text_file.parent().unwrap()).unwrap();
    std::fs::File::create(&text_file).unwrap();
}

fn compare_overhead(binary_name: &String, instrument_type: &str, args: &[String], read_input_from_file: bool) {
    
    CsvFile::instance().clear();
    CsvFile::instance().set_input_file_name(args.join(", "));
    CsvFile::instance().set_input_type(MemoryState::Good);
    CsvFile::instance().set_snapshot_frequency(wasm_runner::SNAPSHOTS_FREQUENCY_GOOD);

    let attestation_wasm_file = format!("{}/binaries/{}/{}{}.wasm", WASM_FILES_FOLDER, binary_name, instrument_type, binary_name);
    let mut wasm_args = Vec::with_capacity(1 + args.len());
    wasm_args.push(attestation_wasm_file.clone());
    wasm_args.extend(args.iter().cloned());

    let mut normal_wasm_program = Wasm::new(&attestation_wasm_file, Mode::ValidationAttestationBaseCase);
    normal_wasm_program.input_type = MemoryState::Good;
    let normal_wasm_time = Instant::now();

    println!("Running file: {}", attestation_wasm_file);
    println!("Args: {:?}", wasm_args);
    match normal_wasm_program.run(&wasm_args, SNAPSHOTS_FOLDER.to_string()) {
        Ok(_) => {
            CsvFile::instance().set_runtime_normal(normal_wasm_time.elapsed());
            CsvFile::instance().set_final_normal_verdict(MemoryState::Good);
        } 
        Err(e) => {
            CsvFile::instance().set_runtime_normal(normal_wasm_time.elapsed());
            let error_string = e.to_string();

            if error_string.contains("!_Exit") || error_string.contains("!exit") {
                CsvFile::instance().set_final_normal_verdict(MemoryState::Good);
            }
            else {
                CsvFile::instance().set_final_normal_verdict(MemoryState::Bad);
            }
        }        
    }

    let mut attestation_wasm_program = Wasm::new(&attestation_wasm_file, Mode::ValidationAttestation);
    attestation_wasm_program.input_type = MemoryState::Good;
    let attestation_wasm_time = Instant::now();

    match attestation_wasm_program.run(&wasm_args, SNAPSHOTS_FOLDER.to_string()) {
        Ok(_) => {
            CsvFile::instance().set_runtime_attestation(attestation_wasm_time.elapsed());
        } 
        Err(_) => {
            CsvFile::instance().set_runtime_attestation(attestation_wasm_time.elapsed());
        }        
    }

    CsvFile::instance().add_csv_entry();
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut read_input_from_file = true;
    if args.len() >= 3 {
        read_input_from_file = match args[2].as_str() {
            "-file" => true,
            "-direct" => false,
            _ => true
        };
    }

    match args[1].as_str() {
        "fuzz" => {
            wasm_runner::memory_snapshot::create_snapshot_folders(&String::from(FUZZER_INPUTS_FOLDER));
            fuzzer::run(&args[3], read_input_from_file);
        },

        "normal" => run_wasm_file(&args[3], &args[4], read_input_from_file),

        "create-snapshots" => {
            wasm_runner::memory_snapshot::create_snapshot_folders(&String::from(SNAPSHOTS_FOLDER));
            collect_memory_snapshots(&args[3], FUZZER_INPUTS_FOLDER, read_input_from_file);
        },

        "convert" => {
            wasm_runner::memory_snapshot::create_snapshot_folders(&String::from(IMAGE_FOLDER));
            convert_rle_to_png(SNAPSHOTS_FOLDER);
        },

        "instrument" => instrument_wasm_files(&args[2]),

        "instrument-binary" => instrument_binaries(&args[2]),

        "instrument-fuzzing" => instrument_wasm_file(&args[2], &args[3], InstrumentMode::Fuzzing),

        "instrument-function" => instrument_wasm_file(&args[2], &args[3], InstrumentMode::LocalFunction),

        "instrument-memory-instr" => instrument_wasm_file(&args[2], &args[3], InstrumentMode::MemoryInstruction),

        "attest" => attest_wasm_file(&args[2].as_str(), &args[3].as_str()),

        "compare-overhead" => {
            let mut csv_file_name = format!("../memory-analyzer/csv-files/binaries/{}/cpu/resnet-local-{}.csv", &args[3], wasm_runner::SNAPSHOTS_FREQUENCY_GOOD);
            CsvFile::instance().set_csv_file_name(csv_file_name);
            CsvFile::instance().create_csv_file();

            compare_overhead(
                &args[3],
                INSTRUMENT_ATTESTATION_LOCAL_PREFIX,
                &args[4..], 
                read_input_from_file
            );

            csv_file_name = format!("../memory-analyzer/csv-files/binaries/{}/cpu/resnet-memory-{}.csv", &args[3], &args[3]);
            CsvFile::instance().clear();
            CsvFile::instance().set_csv_file_name(csv_file_name);
            CsvFile::instance().create_csv_file();

            compare_overhead(
                &args[3],
                INSTRUMENT_ATTESTATION_MEMORY_PREFIX,
                &args[4..], 
                read_input_from_file
            );

            csv_file_name = format!("../memory-analyzer/csv-files/binaries/{}/cpu/resnet-import-{}.csv", &args[3], &args[3]);
            CsvFile::instance().clear();
            CsvFile::instance().set_csv_file_name(csv_file_name);
            CsvFile::instance().create_csv_file();

            compare_overhead(
                &args[3],
                INSTRUMENT_ATTESTATION_IMPORT_PREFIX,
                &args[4..], 
                read_input_from_file
            );
        }

        "validate" => {
            let csv_file_name = format!("fuzzer-output/local-{}.csv", &args[3]);
            CsvFile::instance().set_csv_file_name(csv_file_name);
            CsvFile::instance().create_csv_file();

            validate_ml_model(
                &args[3].as_str(),
                INSTRUMENT_ATTESTATION_LOCAL_PREFIX,
                format!("{}/{}", VALIDATION_FOLDER, &args[3].as_str()).as_str(), 
                read_input_from_file
            );
        },

        "compare" => compare_memory_snapshots(&args[2], &args[3]),

        _ => print!("{}", console_messages::PROGRAM_MODES)
    }
}