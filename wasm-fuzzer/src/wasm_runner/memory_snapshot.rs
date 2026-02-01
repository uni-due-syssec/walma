use std::fs::{self, File, OpenOptions};
use std::io::{Write, Read, BufWriter};
use std::sync::Arc;
use png::{Encoder, ColorType};
use rayon::prelude::*;
use sha2::{Sha256, Digest};

use crate::wasm_runner::socket_client;

const IMAGE_WIDTH: usize = 256;

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum MemoryState {
    Good,
    Bad
}

pub fn create_snapshot_folders(path: &String) {
    let good_folder = String::from(path.to_owned() + "/good");
    let bad_folder = String::from(path.to_owned() + "/bad");

    fs::create_dir_all(good_folder).unwrap();
    fs::create_dir_all(bad_folder).unwrap();
}

pub fn create_memory_snapshot(input: &[u8], folder_path: &String, memory_state: MemoryState) {
    
    let mut hasher = Sha256::new();
    hasher.update(input);
    let input_hash = hasher.finalize_reset();

    let file_name = create_memory_snapshot_name(folder_path, memory_state, hex::encode(input_hash));
    let mut file = OpenOptions::new()
        .write(true)
        .append(false)
        .create(true)
        .open(file_name)
        .unwrap();

    let thread_count = 6;
    let chunk_size = input.len() / thread_count;
    let thread_input: Arc<Vec<u8>> = Arc::new(input.to_vec());
    
    let results: Vec<String> = thread_input
        .par_chunks(chunk_size)
        .map(|chunk| rle_encoding(chunk))
        .collect();

    for result in results {
        file.write_all(result.as_bytes()).unwrap();
    }
}

pub fn compare_memory_snapshots(file1: &str, file2: &str) -> bool {
    let mut file1 = File::open(file1).map_err(|e| format!("Failed to open file1: {}", e)).unwrap();
    let mut file2 = File::open(file2).map_err(|e| format!("Failed to open file2: {}", e)).unwrap();
    
    let mut input1 = String::from("");
    let mut input2 = String::from("");

    file1.read_to_string(&mut input1).unwrap();
    file2.read_to_string(&mut input2).unwrap();

    let thread1 = std::thread::spawn(move || rle_decoding(&input1));
    let thread2 = std::thread::spawn(move || rle_decoding(&input2));

    let decoded1: Vec<u8> = thread1.join().unwrap();
    let decoded2: Vec<u8> = thread2.join().unwrap();

    return  decoded1 == decoded2;
}

pub fn analyze_memory(input: &[u8]) -> MemoryState {
    let mut filtered_input: Vec<u8> = input
        .par_iter()
        .cloned()
        .filter(|&x| x != 0)
        .collect();
    filtered_input.append(&mut vec![0; IMAGE_WIDTH - (filtered_input.len() % IMAGE_WIDTH)]); 

    let clien_socket = socket_client::SocketClient::instance();
    clien_socket.send(filtered_input.as_slice()).expect("Failed to send memory to validator");
    let result: String = clien_socket.receive().expect("Failed to receive message from validator");

    match result.as_str() {
        "bad" => return MemoryState::Bad,
        "BAD" => return MemoryState::Bad,
        "good" => return MemoryState::Good,
        "GOOD "=> return MemoryState::Good,
        _ => eprintln!("Unable to understand validators response '{}'", result)
    }
    
    return MemoryState::Bad;
}

pub fn rle_to_png(input_file: &str, output_file: &str) {
    let mut decoded_input: Vec<u8> = rle_decoding_filtered(&fs::read_to_string(input_file).expect("Failed to read rle text file"));
    decoded_input.append(&mut vec![0; IMAGE_WIDTH - (decoded_input.len() % IMAGE_WIDTH)]); 
    let height: usize = decoded_input.len() / IMAGE_WIDTH;

    let file = File::create(output_file).expect("Failed to create file");
    let writer = BufWriter::new(file);

    let mut encoder = Encoder::new(writer, IMAGE_WIDTH as u32, height as u32);
    encoder.set_color(ColorType::Grayscale);
    encoder.set_compression(png::Compression::Fast);

    let mut png_writer = encoder.write_header().expect("Failed to write PNG header");
    png_writer
        .write_image_data(&decoded_input)
        .expect("Failed to write PNG data");
}

fn create_memory_snapshot_name(folder_path: &String, memory_state: MemoryState, input_hash: String) -> String {
    let mut file_name = String::from(folder_path);
    
    match memory_state {
        MemoryState::Good => file_name.push_str("/good/"),
        MemoryState::Bad => file_name.push_str("/bad/"),
    }

    file_name.push_str(input_hash.as_str());
    file_name.push_str(".txt");

    return file_name;
}

fn rle_encoding(input: &[u8]) -> String {
    let mut encoded_input = String::from("");
    let mut prev_byte = input[0];
    let mut count: u32 = 1;

    encoded_input.reserve(input.len() * 4);
    for &byte in &input[1..] {
        if byte == prev_byte {
            count += 1;
        } 
        else {
            encoded_input.push_str(&format!("{}:{} ", prev_byte, count));
            prev_byte = byte;
            count = 1;
        }
    }

    encoded_input.push_str(&format!("{}:{} ", prev_byte, count));
    return encoded_input;
}

fn rle_decoding(input: &String) -> Vec<u8> {
    let mut decoded_input: Vec<u8>  = Vec::with_capacity(input.len() * 40);

    for element in input.split(" ") {
        if element == "" {
            continue;
        }

        let byte_and_amount: Vec<&str> = element.split(":").collect();
        let byte: u8 = byte_and_amount[0].parse().expect("Failed to parse number");
        let amount: usize = byte_and_amount[1].parse().expect("Failed to parse number");

        decoded_input.append(&mut vec![byte; amount]);
    }

    return decoded_input;
}

fn rle_decoding_filtered(input: &String) -> Vec<u8> {
    let mut decoded_input: Vec<u8>  = Vec::with_capacity(input.len() * 40);

    for element in input.split(" ") {
        if element == "" {
            continue;
        }

        let byte_and_amount: Vec<&str> = element.split(":").collect();
        let byte: u8 = byte_and_amount[0].parse().expect("Failed to parse number");
        let amount: usize = byte_and_amount[1].parse().expect("Failed to parse number");

        if byte == 0 {
            continue;
        }
        decoded_input.append(&mut vec![byte; amount]);
    }

    return decoded_input;
}
