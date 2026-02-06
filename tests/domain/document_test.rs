use sandakan::domain::ContentType;

#[test]
fn given_pdf_mime_when_parsing_then_returns_pdf_content_type() {
    assert_eq!(
        ContentType::from_mime("application/pdf"),
        Some(ContentType::Pdf)
    );
}

#[test]
fn given_audio_mime_when_parsing_then_returns_audio_content_type() {
    assert_eq!(
        ContentType::from_mime("audio/mpeg"),
        Some(ContentType::Audio)
    );
}

#[test]
fn given_unknown_mime_when_parsing_then_returns_none() {
    assert_eq!(ContentType::from_mime("application/unknown"), None);
}
