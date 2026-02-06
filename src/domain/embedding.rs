#[derive(Debug, Clone, PartialEq)]
pub struct Embedding {
    pub values: Vec<f32>,
}

impl Embedding {
    pub fn new(values: Vec<f32>) -> Self {
        Self { values }
    }

    pub fn dimensions(&self) -> usize {
        self.values.len()
    }

    pub fn cosine_similarity(&self, other: &Self) -> f32 {
        if self.values.len() != other.values.len() {
            return 0.0;
        }

        let dot_product: f32 = self
            .values
            .iter()
            .zip(other.values.iter())
            .map(|(a, b)| a * b)
            .sum();

        let magnitude_a: f32 = self.values.iter().map(|x| x * x).sum::<f32>().sqrt();
        let magnitude_b: f32 = other.values.iter().map(|x| x * x).sum::<f32>().sqrt();

        if magnitude_a == 0.0 || magnitude_b == 0.0 {
            return 0.0;
        }

        dot_product / (magnitude_a * magnitude_b)
    }
}
