# Overview

Overview of project...

## Requirements

- [Rust](https://rust-lang.org/learn/get-started/)
- [libtorch 2.9.0](https://pytorch.org/get-started/locally/)
  - In .bashrc add:

    ```text
    export LIBTORCH=.../libtorch/
    export LIBTORCH_INCLUDE=.../libtorch/
    export LIBTORCH_LIB=.../libtorch/
    export LD_LIBRARY_PATH=.../libtorch/lib
    ```

  - If cuda does not work with libtorch, create a python venv and install pytorch 2.9.0 via pip. Add these exports to .bashrc:

    ```text
    export LIBTORCH=.../.venv/lib/python3.10/site-packages/torch
    export LD_LIBRARY_PATH=${LIBTORCH}/lib:$LD_LIBRARY_PATH
    ```

- [OpenVINO Base Package](https://docs.openvino.ai/2025/get-started/install-openvino.html?PACKAGE=OPENVINO_BASE&VERSION=v_2025_4_0&OP_SYSTEM=LINUX&DISTRIBUTION=APT)

## Recommended File Structure

```text
wasm-fuzzer
└── wasm_files
    ├── asan
    │   ├── abc2mtex.wasm
    │   └── jbig2dec.wasm
    ├── corpus
    │   ├── abc2mtex
    │   │   ├── input1.abc
    │   │   └── input2.abc
    │   └── jbig2dec
    │       ├── input1.jb2
    │       └── input2.jb2
    ├── no-asan
    │   ├── abc2mtex.wasm
    │   └── jbig2dec.wasm
    └── validation
        ├── abc2mtex
        │   ├── bad
        │   └── good
        └── jbig2dec
            ├── bad
            └── good
```

This structure is recommended for organizing your project files:

- **asan/**: Contains WebAssembly binaries compiled with AddressSanitizer enabled. These binaries are used for fuzzing to detect memory-related bugs.
- **corpus/**: Includes sample input files used by the fuzzer. Each subdirectory should be named after the corresponding WebAssembly binary and contain relevant input examples for mutation and generation of new test cases.
- **no-asan/**: Contains WebAssembly binaries compiled without AddressSanitizer. These are used for attestation and validation after fuzzing.
- **validation/**: Contains inputs genereated by the fuzzer, which are used for evaluating the trained model. Each subdirectory corresponds to a specific WebAssembly program and is split into:
  - **bad/**: Inputs that previously caused crashes or invalid behavior during fuzzing.
  - **good/**: Inputs that execute successfully without triggering crashes or memory errors.

## Fuel Limiting Execution

To prevent infinite execution of the WebAssembly binary, fuel consumption has been integrated into the runtime. You can adjust the execution limit by modifying the `FUEL_AMOUNT` variable in `wasm_runner.rs`.

## Instrumenting WASM Files

Instrumentation is performed using a single command that automatically **generates multiple instrumented variants** of a given WebAssembly binary. Each variant corresponds to a different instrumentation strategy and is written to disk as a separate file.

### Comand

```console
cargo run instrument <wasm_binary_name>
```

Example:

```console
cargo run instrument abc2mtex
```

This command expects the following input files to exist:

- `wasm_files/asan/<wasm_binary_name>.wasm`
- `wasm_files/no-asan/<wasm_binary_name>.wasm`

### Generated Output Files

For each invocation, the instrumenter produces four instrumented binaries, one per instrumentation mode:

| Mode                               | Purpose                             | Input   | Output Prefix         | Output Location       |
| ---------------------------------- | ----------------------------------- | ------- | --------------------- | --------------------- |
| **Fuzzing**                        | Coverage-guided fuzzing             | ASAN    | `fuzzing-`            | `wasm_files/asan/`    |
| **Local Function Attestation**     | Snapshot at function entry          | no-ASAN | `attestation-local-`  | `wasm_files/no-asan/` |
| **Imported Function Attestation**  | Snapshot after imported calls       | no-ASAN | `attestation-import-` | `wasm_files/no-asan/` |
| **Memory Instruction Attestation** | Snapshot after memory-modifying ops | no-ASAN | `attestation-memory-` | `wasm_files/no-asan/` |

Example output for abc2mtex:

```text
wasm_files
├── asan
│   ├── abc2mtex.wasm
│   └── fuzzing-abc2mtex.wasm
└── no-asan
    ├── abc2mtex.wasm
    ├── attestation-local-abc2mtex.wasm
    ├── attestation-import-abc2mtex.wasm
    └── attestation-memory-abc2mtex.wasm
```

### Instrumentation Modes

#### Fuzzing Instrumentation

The fuzzing variant injects lightweight coverage feedback to guide input generation.

- Inserts calls to the imported Rust function `update_coverage`
- Instrumentation points include:
  - After every `br`, `br_if`, `if`, and `else`
  - At the beginning of every local function
- Each instrumentation site passes a unique integer identifier
- Coverage data is accumulated in the `SIGNALS` array in `fuzzer.rs`

This binary **must** be compiled with AddressSanitizer and is used exclusively during fuzzing.

#### Attestation Instrumentation

All attestation variants call the same imported Rust function:

```rust
create_memory_snapshot
```

Only **no-ASAN** binaries are used for attestation to avoid sanitizer-induced memory layout changes.
Instrumentaion modes:

- **Local Function**: Insert call at the beginning of every local function
- **Imported Function**: Insert call after each call to an imported function
- **Memory Instruction**: Insert call after every memory-modifying instructions, including:
  - `store`
  - `memory.copy`
  - `memory.fill`
  - `memory.init`
  - `memory.grow`

## Fuzzing

The fuzzer uses a coverage-guided approach based on LibAFL and executes an AddressSanitizer-instrumented WebAssembly binary with additional coverage instrumentation.

### Command

```console
cargo run fuzz (-file | -direct) <program_name>
```

The input mode flag determines how fuzzed inputs are passed to the WebAssembly program:

- `-file`: Each fuzzed input is written to a temporary file and the file path is passed as a WASM argument.
- `-direct`: The fuzzed input is passed directly as a UTF-8 decoded command-line argument.

Example:

```console
cargo run fuzz -file abc2mtex
```

This command launches the fuzzer on the specified WebAssembly binary, which should be compiled with AddressSanitizer and [instrumented for fuzzing](#for-fuzzing). The binary must be located in the `wasm_files/asan/` directory.

It will create an `inputs` directory under `fuzzer-output`, where test cases are categorized based on execution outcome:

```

fuzzer-output
└── inputs
    ├── bad   # Inputs that caused a crash
    └── good  # Inputs that executed successfully
```

### Configuring the Fuzzer

Before running the fuzzer, you need to update the following variables in `fuzzer.rs`:

- `input_file` (line 60 in fuzzer.rs) and : Set the input file name with the correct extension expected by the target WASM program.
- `output_file_ending` (line 61 in fuzzer.rs): Set the expected output file extension.
- `WASM_ARGS` (line 27 in main.rs): Specify the command-line arguments for the WASM binary. The first argument should always be the path to the input file. The template supprorts two placeholders:
  - `WASM_FILE` (reqired): Always replaced with the absolute path to the instrumented WebAssembly binary being executed.
  - `INPUT_FILE` (optional): Resolved based on the selected input mode:
    - With `-file`, it is replaced by the path to a temporary file containing the fuzzed input.
    - With `-direct`, it is replaced by the contents of the fuzzed input passed directly as a string argument.

> **Note:**: Don’t forget to adjust the [fuel limit](#fuel-limiting-execution) to control execution time and prevent infinite loops.

## Generating Memory Snapshots

Memory snapshots are generated by executing an attestation-instrumented WebAssembly binary on inputs collected during fuzzing.

### Command

```console
cargo run create-snapshots (-file | -direct) <path_to_wasm_binary>
```

Example:

```console
cargo run create-snapshots -file wasm_files/no-asan/instr-local-abc2mtex.wasm
```

This command uses the [attestation-instrumented](#for-attestation) WebAssembly binary (compiled **without** AddressSanitizer) and processes the inputs previously generated by the fuzzer. The resulting memory snapshots are saved in the following directory structure:

```text
fuzzer-output
└── snapshots
    ├── bad   # Snapshots from crashing inputs
    └── good  # Snapshots from successfully executed inputs
```

Snapshot generation expects fuzzing outputs to exist under:

```text
fuzzer-output
└── inputs
    ├── bad  
    └── good
```

### Controlling Snapshot Frequency

To control how many snapshots are generated, you can modify the following variables in `wasm_runner.rs`:

- `SNAPSHOTS_FREQUENCY_BAD`: Capture a snapshot every _n_ function calls for inputs that cause crashes.
- `SNAPSHOTS_FREQUENCY_GOOD`: Capture a snapshot every _n_ function calls for inputs that execute successfully.

Since crashing inputs usually result in shorter execution, they naturally produce fewer snapshots. These two variables allow you to balance the number of snapshots between good and bad inputs.

## Converting Memorysnapshot Textfiles into Images

To convert the generated memory snapshot text files into images, run:

```console
cargo run convert
```

This command processes all snapshot files located in `fuzzer-output/snapshots` and saves the resulting images in the `fuzzer-output/images` directory. The conversion is done automatically for both good and bad input snapshots.

## Validating ML Model

To validate the machine learning attestation model, run:

```console
cargo run validate (-file | -direct) <wasm_binary_name>
```

Example:

```console
cargo run validate -file abc2mtex
```

This command runs the **attestation-instrumented** version of the WebAssembly binary twice on all inputs located in the `wasm_files/validation` folder:

- The **first run** executes without the ML-based validator to collect baseline execution metrics.
- The **second run** enables the ML-based validator, which analyzes the generated memory snapshots for attestation.

A CSV file containing the validation results will be generated in the `fuzzer-output` directory. You can analyze this file using the provided Jupyter notebook located at `memory-analyzer/src/notebooks/csv_visualizer.ipynb`

### Configuration Tips

Before running validation, make sure to adjust the following parameters in wasm_runner.rs:

- `FUEL_AMOUNT`: Controls the execution time limit for the WASM binary.
- `SNAPSHOTS_FREQUENCY_BAD`: Snapshot frequency for bad inputs.
- `SNAPSHOTS_FREQUENCY_GOOD`: Snapshot frequency for good inputs.

> 💡 It’s recommended to configure the snapshot frequency so that **each input produces at least 10 snapshots** for meaningful validation results.
