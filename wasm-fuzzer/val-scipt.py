import json
import subprocess
import copy

instrument_args = ["target/release/wasm-fuzzer", "instrument-binary"]
run_args = ["target/release/wasm-fuzzer", "compare-overhead"]

def run_binaries(binaries, instrument: bool = False):
    for binary, args in binaries.items():
        args_list = args[0].get("argv", [])
        
        print("\n\n")

        if instrument:
            instr_process = copy.deepcopy(instrument_args)
            instr_process.append(binary)
            print(instr_process)
            subprocess.run(instr_process)

        uses_file = False
        for arg in args_list:
            if "/" in arg:
                uses_file = True

        validation_process = copy.deepcopy(run_args)
        if uses_file:
            validation_process.append("-file")
        else:
            validation_process.append("-direct")

        validation_process.extend(args_list)
        print(f"\n{validation_process}")
        subprocess.run(validation_process)

if __name__ == "__main__":
    
    with open("wasm_files/binaries/benchmarks.json", "r") as f:
        binaries = json.load(f)
    
    run_binaries(binaries)
    
