#![allow(dead_code)]
use super::memory_snapshot::MemoryState;

use once_cell::sync::Lazy;
use tch::{CModule, Device, Kind, Tensor};
use std::error::Error;

const IMAGE_WIDTH: usize = 256;

static MODEL: Lazy<Model> = Lazy::new(|| {
    Model::new("wasm_files/models/general-resnet.pt").expect("Failed to load TorchScript model")
});

pub struct Model {
    model: CModule,
    device: Device,
}

impl Model {
    fn new(model_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let device = Device::cuda_if_available();
        let model = CModule::load_on_device(model_path, device)?;
        print!("Model loaded on: {:?}\n", device);

        Ok(Self { model, device })
    }

    pub fn instance() -> &'static Model {
        &MODEL
    }

    fn preprocess_resnet(&self, byte_array: &[u8]) -> Result<Tensor, Box<dyn Error>> {
        // Pad byte array
        let mut byte_vector: Vec<u8> = byte_array.to_vec();
        let padding_remainder = byte_vector.len() % IMAGE_WIDTH;

        if padding_remainder != 0 {
            byte_vector.append(&mut vec![0; IMAGE_WIDTH - padding_remainder]);
        }

        // Convert to grayscale image
        let height = byte_vector.len() / IMAGE_WIDTH;
        let tensor = Tensor::from_slice(&byte_vector)
            .to_kind(Kind::Float)
            .view([height as i64, IMAGE_WIDTH as i64]);

        // Normalize: from [0, 255] → [0, 1]
        let tensor = tensor / 255.0;

        // Adjust dimensions to [1, 1, height, width]
        Ok(tensor.unsqueeze(0).unsqueeze(0).to_device(self.device))
    }

    fn preprocess_vgg(&self, byte_array: &[u8]) -> Result<Tensor, Box<dyn Error>> {
        // Pad byte array
        let mut byte_vector: Vec<u8> = byte_array.to_vec();
        let padding_remainder = byte_vector.len() % IMAGE_WIDTH;

        if padding_remainder != 0 {
            byte_vector.append(&mut vec![0; IMAGE_WIDTH - padding_remainder]);
        }

        // Convert to grayscale image
        let height = byte_vector.len() / IMAGE_WIDTH;
        let tensor = Tensor::from_slice(&byte_vector)
            .to_kind(Kind::Float)  // This ensures float32, not float64
            .view([height as i64, IMAGE_WIDTH as i64]);

        // Normalize: from [0, 255] → [0, 1]
        let tensor = tensor / 255.0;

        // Convert grayscale to 3-channel RGB (duplicate channel)
        let tensor = tensor.unsqueeze(0); // Add channel dim: [1, H, W]
        let tensor = tensor.repeat(&[3, 1, 1]); // Repeat to 3 channels: [3, H, W]

        // Add batch dimension for resizing: [1, 3, H, W]
        let tensor = tensor.unsqueeze(0);

        // Resize to 224x224 using upsample_bilinear2d
        let tensor = tensor.upsample_bilinear2d(
            &[224, 224], 
            false,  // align_corners
            None,   // scales_h
            None    // scales_w
        ).to_kind(Kind::Float);  // Ensure float32 after resize

        // Normalize with ImageNet mean and std (as float32)
        let mean = Tensor::from_slice(&[0.485_f32, 0.456_f32, 0.406_f32])
            .view([1, 3, 1, 1])
            .to_device(self.device);
        let std = Tensor::from_slice(&[0.229_f32, 0.224_f32, 0.225_f32])
            .view([1, 3, 1, 1])
            .to_device(self.device);
        
        let tensor = (tensor.to_device(self.device) - mean) / std;

        Ok(tensor)
    }

    pub fn predict(&self, byte_array: &[u8]) -> Result<MemoryState, Box<dyn Error>> {
        let input = self.preprocess_resnet(byte_array)?;
        let output = self.model.forward_ts(&[input])?;

        // Resnet
        let prob = output.sigmoid();
        let value = f64::from(prob.double_value(&[0])); 

        Ok(
            if value >= 0.5 {
                MemoryState::Good
            } 
            else {
                MemoryState::Bad
            }
        )

        // VGG
        // let predicted_class = output.argmax(1, false).int64_value(&[0]);
    
        // Ok(if predicted_class == 1 {
        //     MemoryState::Good
        // } else {
        //     MemoryState::Bad
        // })
    }
}