# Overview

This project provides a secure machine-learning inference server capable of running VGG and custom ResNet-based models either inside a Gramine enclave or natively on the host.
The server loads a pre-trained PyTorch model, listens on a TCP socket, receives raw image bytes, performs inference, and returns a classification result.

The primary use case is model attestation and protected inference, where sensitive ML models and data must be isolated from the host environment.

## Requirements

- [Gramine](https://gramine.readthedocs.io/en/latest/installation.html)
- [Python 3.12](https://www.python.org/downloads/)
- gcc
- make

## Running with Gramine (Enclave Mode)

1. Build the enclave:

    ```bash
    cd enclave
    make
    ```

2. Start the inference server inside the enclave:

    ```bash
    gramine-direct ./enclave ./src/main.py attest <model-name>
    ```

    Example:

    ```bash
    gramine-direct ./enclave ./src/main.py attest general-resnet
    ```

## Running without Gramine (Native Mode)

1. Create and activate a virtual environment:

    ```bash
    cd enclave
    python3 -m venv .venv
    source .venv/bin/activate
    ```

2. Install dependencies:

    ```bash
    pip3 install torch torchvision torchaudio --index-url https://download.pytorch.org/whl/cpu
    ```

3. Start the inference server:

    ```bash
    python3 src/main.py attest <model-name>
    ```

    Example:

    ```bash
    python3 src/main.py attest general-resnet
    ```

## Exporting Models

Models can be exported to TorchScript and ONNX formats.

```bash
python3 src/main.py export <model-name>
```

Example:

```bash
python3 src/main.py export general-resnet
```

Exports are written to:

```text
models/tch-onnx/
├── <model-name>.pt
└── <model-name>.onnx
```
