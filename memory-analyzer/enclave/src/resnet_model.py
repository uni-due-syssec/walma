import torch
import torch.nn as nn

from torchvision import transforms
from PIL import Image

class SEBlock(nn.Module):
    def __init__(self, channels, reduction=16):
        super().__init__()
        reduced = max(channels // reduction, 8)

        self.pool = nn.AdaptiveAvgPool1d(1)
        self.fc = nn.Sequential(
            nn.Conv1d(channels, reduced, kernel_size=1, bias=False),
            nn.ReLU(inplace=True),
            nn.Conv1d(reduced, channels, kernel_size=1, bias=False),
            nn.Sigmoid(),
        )

    def forward(self, x):
        scale = self.pool(x)
        scale = self.fc(scale)
        return x * scale
        
class ResidualSEBlock(nn.Module):
    def __init__(self, in_channels, out_channels, kernel_size=3, stride=1,
                 dropout_p=0.3, reduction=16):
        super().__init__()
        padding = kernel_size // 2

        self.conv1 = nn.Conv1d(in_channels, out_channels,
                               kernel_size=kernel_size,
                               stride=stride,
                               padding=padding,
                               bias=False)
        self.gn1 = nn.GroupNorm(num_groups=min(32, max(1, out_channels // 4)),
                                num_channels=out_channels)

        self.relu = nn.ReLU(inplace=True)
        self.dropout = nn.Dropout(p=dropout_p)

        self.conv2 = nn.Conv1d(out_channels, out_channels,
                               kernel_size=kernel_size,
                               padding=padding,
                               bias=False)
        self.gn2 = nn.GroupNorm(num_groups=min(32, max(1, out_channels // 4)),
                                num_channels=out_channels)

        self.se = SEBlock(out_channels, reduction=reduction)

        if in_channels != out_channels or stride != 1:
            self.shortcut = nn.Sequential(
                nn.Conv1d(in_channels, out_channels, kernel_size=1,
                          stride=stride, bias=False),
                nn.GroupNorm(num_groups=min(32, max(1, out_channels // 4)),
                             num_channels=out_channels)
            )
        else:
            self.shortcut = nn.Identity()

    def forward(self, x):
        identity = self.shortcut(x)

        out = self.conv1(x)
        out = self.gn1(out)
        out = self.relu(out)
        out = self.dropout(out)

        out = self.conv2(out)
        out = self.gn2(out)

        out = self.se(out)

        out += identity
        out = self.relu(out)
        return out

class MemoryClassifier(nn.Module):
    def __init__(self, dropout_p=0.3, se_reduction=16):
        super().__init__()

        self.input_conv = nn.Conv1d(
            in_channels=256,
            out_channels=48,
            kernel_size=7,
            stride=2,
            padding=3,
            bias=False
        )

        self.gn = nn.GroupNorm(6, 48)
        self.relu = nn.ReLU(inplace=True)
        self.dropout = nn.Dropout(dropout_p)

        self.pool = nn.MaxPool1d(kernel_size=3, stride=2, padding=1)

        self.layer1 = self._make_layer(48, 64, num_blocks=2, stride=1,
                                       dropout_p=dropout_p,
                                       reduction=se_reduction)
        self.layer2 = self._make_layer(64, 128, num_blocks=2, stride=2,
                                       dropout_p=dropout_p,
                                       reduction=se_reduction)
        self.layer3 = self._make_layer(128, 256, num_blocks=2, stride=2,
                                       dropout_p=dropout_p,
                                       reduction=se_reduction)
        self.layer4 = self._make_layer(256, 256, num_blocks=2, stride=2,
                                       dropout_p=dropout_p,
                                       reduction=se_reduction)

        self.global_pool = nn.AdaptiveAvgPool1d(1)

        self.classifier = nn.Sequential(
            nn.Dropout(dropout_p),
            nn.Linear(256, 128),
            nn.ReLU(inplace=True),
            nn.Dropout(dropout_p),
            nn.Linear(128, 1)
        )

        self._init_weights()

    def _make_layer(self, in_c, out_c, num_blocks, stride, dropout_p, reduction):
        layers = [
            ResidualSEBlock(in_c, out_c, stride=stride,
                               dropout_p=dropout_p,
                               reduction=reduction)
        ]
        for _ in range(1, num_blocks):
            layers.append(
                ResidualSEBlock(out_c, out_c,
                                  stride=1,
                                  dropout_p=dropout_p,
                                  reduction=reduction)
            )
        return nn.Sequential(*layers)

    def _init_weights(self):
        for m in self.modules():
            if isinstance(m, nn.Conv1d):
                nn.init.kaiming_normal_(m.weight, mode='fan_out',
                                        nonlinearity='relu')
            elif isinstance(m, (nn.GroupNorm, nn.BatchNorm1d)):
                nn.init.constant_(m.weight, 1)
                nn.init.constant_(m.bias, 0)
            elif isinstance(m, nn.Linear):
                nn.init.kaiming_normal_(m.weight)
                nn.init.constant_(m.bias, 0)

    def forward(self, x):
        
        if x.dim() == 3:
            
            x = x.unsqueeze(1)

        x = x.squeeze(1).permute(0, 2, 1)

        x = self.input_conv(x)
        x = self.gn(x)
        x = self.relu(x)
        x = self.dropout(x)
        x = self.pool(x)

        x = self.layer1(x)
        x = self.layer2(x)
        x = self.layer3(x)
        x = self.layer4(x)

        x = self.global_pool(x)
        x = x.squeeze(-1)

        logits = self.classifier(x)
        return logits

class ResNet():
    CLASSES = ["bad", "good"]
    IMAGE_WIDTH = 256

    transform = transforms.Compose([
        transforms.Grayscale(num_output_channels=1),
        transforms.ToTensor(),
    ])

    def __init__(self, path_weights: str):
        self.device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
        self.model = self.__load_model(path_weights)

    def __load_model(self, path_weights: str) -> MemoryClassifier:
        
        model = MemoryClassifier()
        model = model.to(self.device)

        model.load_state_dict(torch.load(f=path_weights, map_location=torch.device(self.device), weights_only=True))
        model.eval()
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

        with torch.inference_mode():
            output = self.model(image)
            prob = torch.sigmoid(output)
            pred = (prob > 0.5).float()
            print(f"Predicted class: {int(pred.item())} — confidence: {prob.item():.4f}")
            return self.CLASSES[int(pred.item())]
    
    def export_model_to_tch(self, model_name: str) -> None:
        self.model.eval()
        scripted_model = torch.jit.script(self.model)
        scripted_model.save(f"./models/tch-onnx/{model_name}.pt")

    def export_model_to_onnx(self, model_name: str) -> None:
        self.model.eval()
        sample_height = 512
        dummy_input = torch.randn(1, sample_height, self.IMAGE_WIDTH).to(self.device)

        torch.onnx.export(
            self.model,
            dummy_input,
            f"./models/tch-onnx/{model_name}.onnx",
            export_params=True,
            opset_version=18,
            do_constant_folding=True,
            input_names=['input'],
            output_names=['output'],
            dynamic_axes={
                'input': {0: 'batch_size', 1: 'height'},
                'output': {0: 'batch_size'}
            },
            dynamo=False
        )
