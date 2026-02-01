use super::memory_snapshot::MemoryState;

use once_cell::sync::Lazy;
use std::io::Write;
use std::time::Duration;
use std::sync::Mutex;
use std::{fs, path::Path};
use std::fs::{File, OpenOptions};

static CSV_FILE: Lazy<Mutex<CsvFile>> = Lazy::new(|| Mutex::new(CsvFile::new()));

pub struct CsvFile {
    
    csv_file_name: String,
    input_type: MemoryState,
    input_file_name: String,
    snapshot_frequency: usize,
    amount_good_attestation_results: u32,
    amount_bad_attestation_results: u32,
    final_attestation_verdict: MemoryState,
    final_normal_verdict: MemoryState,
    runtime_normal: Duration,
    amount_instructions_normal: u64,
    runtime_attestation: Duration,
    amount_instructions_attestation: u64,
    inference_times: Vec<u128>,
    final_verdict_time: u32,
    memory_sizes: Vec<usize>
}

impl CsvFile {
    fn new() -> Self {
        return Self {
            csv_file_name: String::from("default.csv"),
            input_type: MemoryState::Bad,
            input_file_name: String::from(""),
            snapshot_frequency: 0,
            amount_good_attestation_results: 0,
            amount_bad_attestation_results: 0,
            final_attestation_verdict: MemoryState::Good,
            final_normal_verdict: MemoryState::Good,
            runtime_normal: Duration::new(0, 0),
            amount_instructions_normal: 0,
            runtime_attestation: Duration::new(0, 0),
            amount_instructions_attestation: 0,
            inference_times: Vec::with_capacity(1200),
            final_verdict_time: 0,
            memory_sizes: Vec::with_capacity(1100)
        };
    }

    pub fn instance() -> std::sync::MutexGuard<'static, CsvFile> {
        return CSV_FILE.lock().unwrap();
    }

    // Create csv file and writes the header
    pub fn create_csv_file(&self) {
        let folder = Path::new(&self.csv_file_name); 
        fs::create_dir_all(folder.parent().unwrap()).unwrap();

        let mut file = File::create(self.csv_file_name.clone()).unwrap();
        file.write_all("
            input_type,
            input_file_name,
            snapshot_frequency,
            amount_good_attestation_results,
            amount_bad_attestation_results,
            final_attestation_verdict,
            final_normal_verdict,
            runtime_normal,
            amount_instructions_normal,
            runtime_attestation,
            amount_instructions_attestation,
            average_inference_time,
            final_verdict_fn_calls,
            average_memory_size"
            .replace("\n", "").replace(" ", "").as_bytes()).unwrap();
    }

    pub fn add_csv_entry(&self) {
        let mut file = OpenOptions::new()
            .write(true)
            .append(true)
            .open(self.csv_file_name.clone())
            .unwrap();

        let inference_time_sum: u128 = self.inference_times.iter().sum();
        let sum_memory_size: usize = self.memory_sizes.iter().sum();
        let entry = format!("\n{:?},{:?},{:?},{:?},{:?},{:?},{:?},{:?},{:?},{:?},{:?},{:?}ms,{:?},{:?}", 
            self.input_type,
            self.input_file_name,
            self.snapshot_frequency,
            self.amount_good_attestation_results,
            self.amount_bad_attestation_results,
            self.final_attestation_verdict,
            self.final_normal_verdict,
            self.runtime_normal,
            self.amount_instructions_normal,
            self.runtime_attestation,
            self.amount_instructions_attestation,
            (inference_time_sum as f64 / self.inference_times.len() as f64),
            self.final_verdict_time,
            (sum_memory_size as f64 / self.memory_sizes.len() as f64)
        );

        file.write_all(entry.as_bytes()).unwrap();
    }

    pub fn clear(&mut self) {
        self.input_type = MemoryState::Bad;
        self.input_file_name = String::from("");
        self.snapshot_frequency = 0;
        self.amount_good_attestation_results = 0;
        self.amount_bad_attestation_results = 0;
        self.final_attestation_verdict = MemoryState::Good;
        self.final_normal_verdict = MemoryState::Good;
        self.runtime_normal = Duration::new(0, 0);
        self.amount_instructions_normal = 0;
        self.runtime_attestation = Duration::new(0, 0);
        self.amount_instructions_attestation = 0;
        self.inference_times.clear();
        self.final_verdict_time = 0;
        self.memory_sizes.clear();
    }

    pub fn set_csv_file_name(&mut self, name: String) {
        self.csv_file_name = name;
    }

    pub fn set_input_type(&mut self, input: MemoryState) {
        self.input_type = input;
    }

    pub fn set_input_file_name(&mut self, name: String) {
        self.input_file_name = name;
    }

    pub fn set_snapshot_frequency(&mut self, freq: usize) {
        self.snapshot_frequency = freq;
    }

    pub fn increase_amount_good_results(&mut self) {
        self.amount_good_attestation_results += 1;
    }

    pub fn increase_amount_bad_results(&mut self) {
        self.amount_bad_attestation_results += 1;
    }

    pub fn set_final_attestation_verdict(&mut self, verdict: MemoryState) {
        self.final_attestation_verdict = verdict;
    }

    pub fn set_final_normal_verdict(&mut self, verdict: MemoryState) {
        self.final_normal_verdict = verdict;
    }

    pub fn set_runtime_normal(&mut self, runtime: Duration) {
        self.runtime_normal = runtime;
    }

    pub fn set_amount_instructions_normal(&mut self, amount: u64) {
        self.amount_instructions_normal = amount;
    }

    pub fn set_runtime_attestation(&mut self, runtime: Duration) {
        self.runtime_attestation = runtime;
    }

    pub fn set_amount_instructions_attestation(&mut self, amount: u64) {
        self.amount_instructions_attestation = amount;
    }

    pub fn add_inference_time(&mut self, time: Duration) {
        self.inference_times.push(time.as_millis());
    }

    pub fn set_final_verdict_time(&mut self, amount: u32) {
        if self.final_verdict_time == 0 {
            self.final_verdict_time = amount;
        }
    }

    pub fn add_memory_size(&mut self, size: usize) {
        self.memory_sizes.push(size);
    }

    pub fn get_memory_sizes_length(&mut self) -> usize {
        return  self.memory_sizes.len();
    }
}