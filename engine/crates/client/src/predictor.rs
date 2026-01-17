use engine::transformer::{Transformer, Config};
use candle_core::{Device, Tensor, DType, IndexOp};
use candle_nn::{VarBuilder, VarMap};

pub struct Predictor {
    model: Transformer,
    device: Device,
}

impl Predictor {
    pub fn new(weights: &[u8]) -> Result<Self, String> {
        let device = Device::Cpu;
        
        // Load safetensors from buffer
        let tensors = candle_core::safetensors::load_buffer(weights, &device)
            .map_err(|e| format!("Failed to load weights: {}", e))?;
            
        let vb = VarBuilder::from_tensors(tensors, DType::F32, &device);
        
        let config = Config::default();
        let model = Transformer::new(&config, vb)
            .map_err(|e| format!("Failed to build model: {}", e))?;

        Ok(Self { model, device })
    }

    pub fn predict(&self, tokens: &[u8]) -> Result<u8, String> {
        if tokens.is_empty() { return Err("Empty tokens".to_string()); }

        let input_u32: Vec<u32> = tokens.iter().map(|&x| x as u32).collect();
        let input_tensor = Tensor::from_vec(input_u32, (1, tokens.len()), &self.device)
            .map_err(|e| format!("Tensor error: {}", e))?;

        let logits = self.model.forward(&input_tensor)
            .map_err(|e| format!("Inference error: {}", e))?;

        let (_b, seq_len, _vocab) = logits.dims3().map_err(|e| e.to_string())?;
        let last_logits = logits.i((0, seq_len - 1, ..)).map_err(|e| e.to_string())?;
        
        let next_token = last_logits.argmax(0).map_err(|e| e.to_string())?
            .to_scalar::<u32>().map_err(|e| e.to_string())?;

        Ok(next_token as u8)
    }
}
