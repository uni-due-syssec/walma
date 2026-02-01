# Commands for wasm-fuzzer

## Fuzzing

cargo run -r fuzz abc2mtex
cargo run -r fuzz pnm2png
cargo run -r fuzz jbig2dec
cargo run -r fuzz pdfresurrect

## Instrumenting WASM File

cargo run -r instrument-attestation wasm_files/no-asan/abc2mtex.wasm wasm_files/no-asan/
cargo run -r instrument-fuzzing wasm_files/asan/abc2mtex.wasm wasm_files/asan/

cargo run -r instrument-attestation wasm_files/no-asan/jbig2dec.wasm wasm_files/no-asan/
cargo run -r instrument-fuzzing wasm_files/asan/jbig2dec.wasm wasm_files/asan/

## Run WASM File normaly

### abc2mtex
cargo run -r normal wasm_files/asan/fuzzing-abc2mtex.wasm wasm_files/corpus/abc2mtex/English.abc
cargo run -r normal wasm_files/no-asan/attestation-abc2mtex.wasm wasm_files/corpus/abc2mtex/English.abc

### pnm2png
cargo run -r normal wasm_files/asan/fuzzing-pnm2png.wasm wasm_files/corpus/pnm2png/basn0g01.pgm out.png
cargo run -r normal wasm_files/asan/pnm2png.wasm wasm_files/corpus/pnm2png/input-too-large-segfault out.png

cargo run -r attest wasm_files/no-asan/attestation-pnm2png.wasm fuzzer-output/validation/bad/d52d2bae4b45210c06b41ebb6f31e874e73260d32365a62c7519b368c352e9eb.pgm
cargo run -r attest wasm_files/no-asan/attestation-pnm2png.wasm fuzzer-output/validation/good/f9de7603e65ffe05c93cd50715ab9f8bad676cdb5633e5d7433112dd4b90cd8f.pgm

### jbig2dec

cargo run -r normal wasm_files/asan/jbig2dec.wasm wasm_files/corpus/jbig2dec/annex-h.jbig2 -t png -o ./out.png

## Generate Memorysnapshot Textfiles

cargo run -r create-snapshots wasm_files/no-asan/attestation-pal2rgb.wasm

## Converting Memorysnapshot Textfiles into Images

cargo run -r convert

## Attest WASM File

cargo run attest wasm_files/no-asan/attestation-abc2mtex.wasm wasm_files/inputs/English.abc

cargo run attest wasm_files/no-asan/attestation-pnm2png.wasm fuzzer-output/validation/bad/d52d2bae4b45210c06b41ebb6f31e874e73260d32365a62c7519b368c352e9eb.pgm
cargo run attest wasm_files/no-asan/attestation-pnm2png.wasm fuzzer-output/validation/good/0f903d115b1a877cdec8d5550dd6112b5effba9593c11a16eef5044c468ae26b.pgm

cargo run attest wasm_files/no-asan/attestation-jbig2dec.wasm fuzzer-output/validation/bad/2b9f525b989d25f3c843c44dc5c44414c084de451f467c972b6c52d7c76ba44b.jb2
cargo run attest wasm_files/no-asan/attestation-jbig2dec.wasm fuzzer-output/validation/good/cd892a20c6c4ddcecaadeb8bf61179f3cf37a41c29031a7692dd4eaf34481971..jb2

cargo run attest wasm_files/no-asan/attestation-jbig2dec.wasm wasm_files/corpus/jbig2dec/poc

## Validating ML Model

cargo run -r validate wasm_files/no-asan/ pnm2png.wasm

## Comparing two Memorysnapshotfiles

cargo run compare ./fuzzer_output/good/6.txt ./fuzzer_output/good/6.txt

## Running wasmtime via Terminal

wasmtime --dir=. wasm_files/no-asan/abc2mtex.wasm wasm_files/inputs/English.abc