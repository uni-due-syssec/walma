from PIL import Image

import torch
import torch.nn as nn

from torchvision import models, transforms

class VGG():
    CLASSES = ["bad", "good"]
    IMAGE_WIDTH = 256

    transform = transforms.Compose([
        transforms.Grayscale(num_output_channels=3),
        transforms.Resize((224, 224)),
        transforms.ToTensor(),
        transforms.Normalize(mean=[0.485, 0.456, 0.406], std=[0.229, 0.224, 0.225])
    ])

    def __init__(self, path_weights: str):
        self.device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
        self.model = self.__load_model(path_weights)
        self.model.to(self.device)

    def __load_model(self, path_weights: str) -> models.vgg.VGG:
        model = models.vgg16(weights="DEFAULT")
        model.classifier[6] = nn.Linear(4096, 2) 
        model.load_state_dict(torch.load(f=path_weights, map_location=torch.device(self.device)))
        print(f"Model ({path_weights}) loaded on {self.device}")

        return model
    
    def __load_image(self, byte_array: bytearray) -> torch.Tensor:
        image = Image.frombytes(
            "L", 
            (self.IMAGE_WIDTH, int(len(byte_array) / self.IMAGE_WIDTH)), 
            bytes(byte_array))
        image = self.transform(image)
        image = image.unsqueeze(0)

        return image
    
    def predict(self, image: bytearray) -> str:
        image = self.__load_image(image).to(self.device)

        self.model.eval()
        with torch.inference_mode():
            output = self.model(image)
            _, prediction = torch.max(output, 1)
            return self.CLASSES[prediction]
        
    def export_model_to_tch(self, model_name: str) -> None:
        self.model.eval()
        scripted_model = torch.jit.script(self.model)
        scripted_model.save(f"./models/tch-onnx/{model_name}.pt")

    def export_model_to_onnx(self, model_name: str) -> None:
        self.model.eval()
        dummy_input = torch.randn(1, 3, 224, 224, device=self.device)

        torch.onnx.export(
            self.model,
            dummy_input,
            f"./models/tch-onnx/{model_name}.onnx",
            export_params=True,
            opset_version=18,
            do_constant_folding=True,
            input_names=["input"],
            output_names=["output"],
            dynamic_axes={
                "input": {0: "batch_size"},
                "output": {0: "batch_size"}
            },
            dynamo=False
        )