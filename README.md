# Walma: Learning to See Memory Corruption in WebAssembly

This repository contains the artifact for the paper *"Walma: Learning to See Memory Corruption in WebAssembly"*.

Walma is a framework for detecting memory corruption and external tampering in WebAssembly applications by classifying snapshots of linear memory using convolutional neural networks (CNNs).

## Overview

WebAssembly's monolithic linear memory model co-locates the stack, heap, and global data without runtime-enforced separation, enabling memory corruption attacks that can escalate to cross-site scripting (XSS) in browsers or go undetected when a malicious host tampers with a module's state. Walma addresses this by treating memory snapshots as images and employing CNNs to classify runtime integrity.

The framework operates in two phases:
1. **Offline Training**: Uses coverage-guided fuzzing with sanitizers to generate labeled datasets of benign and corrupted memory states
2. **Online Inference**: Performs runtime attestation via a decoupled attester-verifier architecture

## Repository Structure

```
.
├── wasm-fuzzer/          # Runtime components (Rust)
│   ├── src/
│   │   ├── main.rs              # Entry point and CLI
│   │   ├── fuzzer.rs            # LibAFL integration for dataset generation
│   │   ├── instrumenter.rs      # Binary instrumentation via walrus
│   │   ├── wasm_runner.rs       # Wasmtime-based execution engine
│   │   └── wasm_runner/
│   │       ├── memory_attester.rs   # Verifier backends (cpu-wasm, cpu-tch, gpu)
│   │       ├── memory_snapshot.rs   # Snapshot capture and RLE compression
│   │       └── ...
│   └── wasm_files/       # Sample Wasm binaries
│
├── memory-analyzer/      # ML infrastructure (Python)
│   ├── enclave/          # Gramine SGX enclave configuration
│   │   ├── src/
│   │   │   ├── resnet_model.py  # Lightweight ResNet (1.9M parameters)
│   │   │   └── vgg_model.py     # VGG-16 baseline (134M parameters)
│   │   └── Makefile
│   └── notebooks/        # Evaluation and visualization
│       ├── csv_visualizer.ipynb
│       └── eval_util.py
│
└── walma.pdf             # Paper
```

## Requirements

### System Dependencies
- Rust 1.70+ with `wasm32-wasi` target
- Python 3.10+
- CUDA 12.0+ (optional, for GPU backend)
- Intel SGX SDK (optional, for enclave deployment)

### Rust Dependencies
- [LibAFL](https://github.com/AFLplusplus/LibAFL) - Coverage-guided fuzzing
- [Wasmtime](https://wasmtime.dev/) - WebAssembly runtime
- [walrus](https://github.com/rustwasm/walrus) - Wasm binary instrumentation
- [tch-rs](https://github.com/LaurentMazare/tch-rs) - PyTorch bindings for Rust

### Python Dependencies
- PyTorch 2.0+
- OpenVINO (for WASI-NN backend)
- NumPy, Pandas, Matplotlib

## Building

### Runtime Components (Rust)

```bash
cd wasm-fuzzer

# Add WebAssembly target
rustup target add wasm32-wasi

# Build in release mode
cargo build --release
```

### ML Models (Python)

```bash
cd memory-analyzer

# Create virtual environment
python -m venv venv
source venv/bin/activate

# Install dependencies
pip install torch torchvision openvino numpy pandas matplotlib jupyter
```

## Usage

### 1. Binary Instrumentation

Instrument a Wasm binary with snapshot hooks:

```bash
./target/release/wasm-fuzzer instrument \
    --input target.wasm \
    --output instrumented.wasm \
    --policy import  # Options: import, local, memory
```

**Instrumentation Policies:**
- `import`: Triggers snapshots before host calls (boundary defense, 1.07x overhead)
- `local`: Instruments internal function entries (1.50x overhead)
- `memory`: Triggers on every memory write (fine-grained, 1.82x overhead)

### 2. Dataset Generation

Generate labeled training data using coverage-guided fuzzing:

```bash
./target/release/wasm-fuzzer fuzz \
    --binary instrumented.wasm \
    --corpus corpus/ \
    --output snapshots/ \
    --timeout 3600
```

Snapshots are automatically labeled using AddressSanitizer as the ground-truth oracle.

### 3. Model Training

Train the CNN classifier:

```bash
cd memory-analyzer/enclave/src

python main.py train \
    --dataset ../snapshots/ \
    --model resnet \
    --output models/resnet.pt \
    --epochs 50
```

### 4. Runtime Attestation

Run with continuous memory attestation:

```bash
./target/release/wasm-fuzzer run \
    --binary instrumented.wasm \
    --model models/resnet.pt \
    --backend cpu-wasm \
    --window-size 5 \
    --threshold 3
```

**Verifier Backends:**
- `cpu-wasm`: In-runtime via WASI-NN/OpenVINO (lowest latency, ~5.3ms)
- `cpu-tch`: Native host via LibTorch (~32ms)
- `gpu`: CUDA acceleration (~27ms, includes PCIe transfer overhead)

### 5. SGX Enclave Deployment

For Scenario A (untrusted cloud host), deploy the verifier in an SGX enclave:

```bash
cd memory-analyzer/enclave
make SGX=1
gramine-sgx ./walma-verifier
```

## Evaluated Applications

The paper evaluates Walma on six real-world applications with known CVEs:

| Application | CVE | Description |
|-------------|-----|-------------|
| abc2mtex | CVE-2004-125 | Text processor |
| flac | CVE-2017-6888 | Audio encoder |
| jbig2dec | CVE-2020-12268 | Image decoder |
| pal2rgb | CVE-2016-10504 | Image converter (OpenJPEG) |
| pdfresurrect | CVE-2019-14267 | PDF manipulator |
| pnm2png | CVE-2018-14550 | Image converter (libpng) |

## Key Results

- **Accuracy**: Up to 100% verdict accuracy on structured workloads (flac, pal2rgb)
- **Overhead**: 1.07x geometric mean for boundary defense (import function policy)
- **Generalization**: Cross-program detection maintains 94.5% accuracy on flac without program-specific training
