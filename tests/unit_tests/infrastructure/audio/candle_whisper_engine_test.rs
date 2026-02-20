use candle_core::{DType, Device};
use sandakan::infrastructure::audio::CandleWhisperEngine;

#[test]
fn given_cpu_device_when_selecting_dtype_then_returns_f32() {
    let dtype = CandleWhisperEngine::select_dtype(&Device::Cpu);
    assert!(matches!(dtype, DType::F32));
}

#[test]
fn given_metal_device_when_selecting_dtype_then_returns_f16() {
    let device = Device::new_metal(0).unwrap_or(Device::Cpu);
    let dtype = CandleWhisperEngine::select_dtype(&device);
    let expected = if device.is_cpu() {
        DType::F32
    } else {
        DType::F16
    };
    assert_eq!(dtype, expected);
}
