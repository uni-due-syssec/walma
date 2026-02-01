import socket
import struct
import sys

from threading import Thread
from vgg_model import VGG
from resnet_model import ResNet

USAGE = """
Usage: main.py <mode> <model_name>

mode:
    attest: Loads the passed ml model for attestation
    export: Exports the passed ml model as torchscipt file
"""

PORT: int = 8080
ADDRESS: str = "127.0.0.1"
MESSAGE_SIZE: int = 4096

def handle_user(c_socket: socket.socket):
    while True:
        
        header = c_socket.recv(8)
        if len(header) < 8:
            print("Error: Could not read the message size")
            break

        message_length = struct.unpack(">Q", header)[0]
        received_data = bytearray() 
        remaining_bytes = message_length

        while remaining_bytes > 0:
            chunk = c_socket.recv(min(MESSAGE_SIZE, remaining_bytes))
            if not chunk:
                print("Connection closed unexpectedly")
                break
            
            received_data.extend(chunk)
            remaining_bytes -= len(chunk)

        prediction = model.predict(received_data)        
        print(f"Sending result: {prediction}")
        c_socket.send(prediction.encode())

def export_model(model_name: str) -> None:
    model_path = f"./models/{model_name}.pth"

    if "vgg" in model_name:
        model = VGG(model_path)
    elif "resnet" in model_name:
        model = ResNet(model_path)
    else:
        print("Model is not recognized. Make sure it is either a VGG or ResNet Model")
        exit(0)

    model.model.eval()
    model.export_model_to_tch(model_name)
    model.export_model_to_onnx(model_name)

if __name__ == "__main__":
    
    if len(sys.argv) < 3:
        print(USAGE)
        exit()
    
    if sys.argv[1] == "export":
        export_model(sys.argv[2])
        exit()
    if sys.argv[1] == "attest":
        
        model_path = f"./models/{sys.argv[2]}.pth"

        if "vgg" in model_path:
            model = VGG(model_path)
        elif "resnet" in model_path:
            model = ResNet(model_path)
        else:
            print("Model is not recognized. Make sure it is either a VGG or ResNet Model")
            exit(0)

        s_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s_socket.bind((ADDRESS, PORT))
        s_socket.listen(5)
        print(f"Server is listening on Port: {PORT}", flush=True)

        client_threads = []
        while True:
            c_socket, address = s_socket.accept()

            thread = Thread(target=handle_user, args=(c_socket,))
            thread.start()
            client_threads.append(thread)
    print(USAGE)
