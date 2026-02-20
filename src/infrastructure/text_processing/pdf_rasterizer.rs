use image::ImageFormat;
use pdfium_render::prelude::*;

use crate::application::ports::FileLoaderError;

use super::local_vlm_pdf_adapter::{MAX_PAGES_DUE_TO_RAM_USAGE, RENDER_DPI};

pub(super) fn rasterize_pages(data: &[u8]) -> Result<Vec<Vec<u8>>, FileLoaderError> {
    let pdfium = Pdfium::new(
        Pdfium::bind_to_system_library()
            .map_err(|e| FileLoaderError::ExtractionFailed(format!("pdfium bind failed: {e}")))?,
    );

    let doc = pdfium
        .load_pdf_from_byte_slice(data, None)
        .map_err(|e| FileLoaderError::ExtractionFailed(format!("pdfium open failed: {e}")))?;

    let page_count = doc.pages().len() as usize;
    let pages_to_render = page_count.min(MAX_PAGES_DUE_TO_RAM_USAGE);

    let mut png_buffers: Vec<Vec<u8>> = Vec::with_capacity(pages_to_render);

    for index in 0..pages_to_render {
        let page = doc.pages().get(index as u16).map_err(|e| {
            FileLoaderError::ExtractionFailed(format!("page {index} access failed: {e}"))
        })?;

        let width = (page.width().value * RENDER_DPI / 72.0) as i32;
        let height = (page.height().value * RENDER_DPI / 72.0) as i32;

        let bitmap = page
            .render_with_config(
                &PdfRenderConfig::new()
                    .set_target_width(width)
                    .set_target_height(height),
            )
            .map_err(|e| {
                FileLoaderError::ExtractionFailed(format!("render page {index} failed: {e}"))
            })?;

        let dynamic_image = bitmap.as_image();
        let mut png_bytes: Vec<u8> = Vec::new();
        dynamic_image
            .write_to(&mut std::io::Cursor::new(&mut png_bytes), ImageFormat::Png)
            .map_err(|e| {
                FileLoaderError::ExtractionFailed(format!("PNG encode page {index} failed: {e}"))
            })?;

        png_buffers.push(png_bytes);
    }

    Ok(png_buffers)
}
