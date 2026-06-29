//! safetensors weight (de)serialization helpers.
//!
//! Trained weights live in a separate HuggingFace model repo; this module reads
//! the on-disk [`safetensors`] format used to transport them.

use anyhow::Result;
use safetensors::SafeTensors;

/// Return the names of every tensor stored in a serialized safetensors buffer.
pub fn tensor_names(buffer: &[u8]) -> Result<Vec<String>> {
    let tensors = SafeTensors::deserialize(buffer)?;
    Ok(tensors.names().into_iter().map(str::to_string).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use safetensors::Dtype;
    use safetensors::serialize;
    use safetensors::tensor::TensorView;

    #[test]
    fn round_trips_tensor_names() {
        let data: Vec<u8> = [0.0f32, 1.0f32]
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        let view = TensorView::new(Dtype::F32, vec![2], &data).expect("view");
        let bytes = serialize(vec![("weight", view)], None).expect("serialize");

        let names = tensor_names(&bytes).expect("read names");
        assert_eq!(names, vec!["weight".to_string()]);
    }
}
