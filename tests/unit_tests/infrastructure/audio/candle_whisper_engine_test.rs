use candle_core::{DType, Device};
use sandakan::infrastructure::audio::CandleWhisperEngine;

#[test]
fn given_cpu_device_when_selecting_dtype_then_returns_f32() {
    let dtype = CandleWhisperEngine::select_dtype(&Device::Cpu);
    assert!(matches!(dtype, DType::F32));
}
