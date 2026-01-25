use candle_core::{DType, Device, IndexOp, Module, Result, Tensor};
use candle_nn::{
    embedding, layer_norm, linear, ops, Activation, Embedding, LayerNorm, Linear, VarBuilder,
};

#[derive(Debug, Clone)]
pub struct Config {
    pub vocab_size: usize,
    pub hidden_size: usize,
    pub num_layers: usize,
    pub num_heads: usize,
    pub max_seq_len: usize,
    pub dropout: f32,
}

impl Config {
    pub fn default() -> Self {
        Self {
            vocab_size: 256, // Byte-level tokenizer (0-255)
            hidden_size: 256,
            num_layers: 4,
            num_heads: 8,
            max_seq_len: 128,
            dropout: 0.1,
        }
    }
}

struct CausalSelfAttention {
    c_attn: Linear,
    c_proj: Linear,
    n_head: usize,
    n_embd: usize,
}

impl CausalSelfAttention {
    fn new(cfg: &Config, vb: VarBuilder) -> Result<Self> {
        let n_embd = cfg.hidden_size;
        let n_head = cfg.num_heads;
        let c_attn = linear(n_embd, 3 * n_embd, vb.pp("c_attn"))?;
        let c_proj = linear(n_embd, n_embd, vb.pp("c_proj"))?;
        Ok(Self {
            c_attn,
            c_proj,
            n_head,
            n_embd,
        })
    }

    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let (b, t, c) = x.dims3()?;
        let qkv = self.c_attn.forward(x)?;
        let qkv = qkv.reshape((b, t, 3, self.n_head, c / self.n_head))?;
        let qkv = qkv.transpose(1, 2)?; // (b, 3, t, h, d)

        let q = qkv.i((.., 0))?.transpose(1, 2)?.contiguous()?; // (b, h, t, d)
        let k = qkv.i((.., 1))?.transpose(1, 2)?.contiguous()?; // (b, h, t, d)
        let v = qkv.i((.., 2))?.transpose(1, 2)?.contiguous()?; // (b, h, t, d)

        // Scaled Dot-Product Attention
        let k_t = k.transpose(2, 3)?.contiguous()?; // (b, h, d, t)
        let scale = (c as f64 / self.n_head as f64).sqrt();
        let att = (q.matmul(&k_t)? / scale)?; // (b, h, t, t)

        // Causal Mask
        let mask = self.create_mask(t, x.device())?;
        let mask = mask.broadcast_as((b, self.n_head, t, t))?;
        let att = att.broadcast_add(&mask)?;
        let att = ops::softmax(&att, 3)?;

        let y = att.matmul(&v)?.contiguous()?; // (b, h, t, d)
        let y = y.transpose(1, 2)?.reshape((b, t, c))?; // (b, t, h, d) -> (b, t, c)
        self.c_proj.forward(&y)
    }

    fn create_mask(&self, size: usize, device: &Device) -> Result<Tensor> {
        // Create a lower triangular mask
        let i = Tensor::arange(0u32, size as u32, device)?.reshape((size, 1))?;
        let j = Tensor::arange(0u32, size as u32, device)?.reshape((1, size))?;

        // Broadcast manually if needed, but usually broadcast_ge handles it.
        // If i.ge(&j) failed, try explicit broadcast.
        let i = i.broadcast_as((size, size))?;
        let j = j.broadcast_as((size, size))?;

        let mask = i.ge(&j)?; // Lower triangular boolean
        let mask = mask.to_dtype(DType::F32)?;
        let mask = ((mask * -1.0)? + 1.0)?; // Invert
        let mask = (mask * -1e9)?;
        Ok(mask)
    }
}

struct MLP {
    c_fc: Linear,
    c_proj: Linear,
}

impl MLP {
    fn new(cfg: &Config, vb: VarBuilder) -> Result<Self> {
        let c_fc = linear(cfg.hidden_size, 4 * cfg.hidden_size, vb.pp("c_fc"))?;
        let c_proj = linear(4 * cfg.hidden_size, cfg.hidden_size, vb.pp("c_proj"))?;
        Ok(Self { c_fc, c_proj })
    }

    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let x = self.c_fc.forward(x)?;
        let x = Activation::Gelu.forward(&x)?;
        self.c_proj.forward(&x)
    }
}

struct Block {
    ln1: LayerNorm,
    attn: CausalSelfAttention,
    ln2: LayerNorm,
    mlp: MLP,
}

impl Block {
    fn new(cfg: &Config, vb: VarBuilder) -> Result<Self> {
        let ln1 = layer_norm(cfg.hidden_size, 1e-5, vb.pp("ln1"))?;
        let attn = CausalSelfAttention::new(cfg, vb.pp("attn"))?;
        let ln2 = layer_norm(cfg.hidden_size, 1e-5, vb.pp("ln2"))?;
        let mlp = MLP::new(cfg, vb.pp("mlp"))?;
        Ok(Self {
            ln1,
            attn,
            ln2,
            mlp,
        })
    }

    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let x = (x + self.attn.forward(&self.ln1.forward(x)?)?)?;
        let x = (&x + self.mlp.forward(&self.ln2.forward(&x)?)?)?;
        Ok(x)
    }
}

pub struct Transformer {
    wte: Embedding,
    wpe: Embedding,
    blocks: Vec<Block>,
    ln_f: LayerNorm,
    lm_head: Linear,
    cfg: Config,
}

impl Transformer {
    pub fn new(cfg: &Config, vb: VarBuilder) -> Result<Self> {
        let wte = embedding(cfg.vocab_size, cfg.hidden_size, vb.pp("wte"))?;
        let wpe = embedding(cfg.max_seq_len, cfg.hidden_size, vb.pp("wpe"))?;

        let mut blocks = Vec::new();
        for i in 0..cfg.num_layers {
            blocks.push(Block::new(cfg, vb.pp(&format!("blocks.{}", i)))?);
        }

        let ln_f = layer_norm(cfg.hidden_size, 1e-5, vb.pp("ln_f"))?;
        let lm_head = linear(cfg.hidden_size, cfg.vocab_size, vb.pp("lm_head"))?;

        Ok(Self {
            wte,
            wpe,
            blocks,
            ln_f,
            lm_head,
            cfg: cfg.clone(),
        })
    }

    pub fn forward(&self, idx: &Tensor) -> Result<Tensor> {
        let (_b, t) = idx.dims2()?;
        let pos = Tensor::arange(0u32, t as u32, idx.device())?;
        let pos = pos.reshape((1, t))?;

        let tok_emb = self.wte.forward(idx)?;
        let pos_emb = self.wpe.forward(&pos)?;

        let mut x = (tok_emb.broadcast_add(&pos_emb))?;

        for block in &self.blocks {
            x = block.forward(&x)?;
        }

        let x = self.ln_f.forward(&x)?;
        let logits = self.lm_head.forward(&x)?;

        Ok(logits)
    }
}
