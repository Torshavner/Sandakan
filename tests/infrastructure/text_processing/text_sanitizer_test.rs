use sandakan::infrastructure::text_processing::sanitize_extracted_text;

#[test]
fn given_text_with_fi_ligature_when_sanitizing_then_decomposes_to_fi() {
    let input = "ﬁnding the ﬁle";
    let result = sanitize_extracted_text(input);
    assert_eq!(result, "finding the file");
}

#[test]
fn given_text_with_fl_ligature_when_sanitizing_then_decomposes_to_fl() {
    let input = "a ﬂood of data";
    let result = sanitize_extracted_text(input);
    assert_eq!(result, "a flood of data");
}

#[test]
fn given_text_with_excessive_newlines_when_sanitizing_then_collapses_to_paragraph_breaks() {
    let input = "paragraph one\n\n\n\n\nparagraph two";
    let result = sanitize_extracted_text(input);
    assert_eq!(result, "paragraph one\n\nparagraph two");
}

#[test]
fn given_text_with_redundant_spaces_when_sanitizing_then_collapses_to_single_space() {
    let input = "hello    world   test";
    let result = sanitize_extracted_text(input);
    assert_eq!(result, "hello world test");
}

#[test]
fn given_empty_text_when_sanitizing_then_returns_empty() {
    assert_eq!(sanitize_extracted_text(""), "");
}

#[test]
fn given_whitespace_only_text_when_sanitizing_then_returns_empty() {
    assert_eq!(sanitize_extracted_text("   \n\n  "), "");
}

#[test]
fn given_text_with_mixed_ligatures_and_whitespace_when_sanitizing_then_normalizes_both() {
    let input = "The ﬁrst   ﬂoor\n\n\n\nSecond ﬂoor";
    let result = sanitize_extracted_text(input);
    assert_eq!(result, "The first floor\n\nSecond floor");
}

#[test]
fn given_text_with_hyphenated_line_break_when_sanitizing_then_merges_word() {
    let input = "This is a process-\ning step";
    let result = sanitize_extracted_text(input);
    assert_eq!(result, "This is a processing step");
}

#[test]
fn given_text_with_intentional_hyphen_when_sanitizing_then_preserves_hyphen() {
    let input = "This is well-known";
    let result = sanitize_extracted_text(input);
    assert_eq!(result, "This is well-known");
}

#[test]
fn given_text_with_list_marker_hyphen_when_sanitizing_then_preserves_list() {
    let input = "Items:\n- first item\n- second item";
    let result = sanitize_extracted_text(input);
    assert_eq!(result, "Items:\n- first item\n- second item");
}

#[test]
fn given_hyphen_with_invisible_trailing_whitespace_when_sanitizing_then_merges_word() {
    let input = "The extraction process- \n ing is complex.";
    let result = sanitize_extracted_text(input);
    assert_eq!(result, "The extraction processing is complex.");
}

#[test]
fn given_hyphen_with_indentation_on_next_line_when_sanitizing_then_merges_word() {
    let input = "Protect the environ-\n    ment today.";
    let result = sanitize_extracted_text(input);
    assert_eq!(result, "Protect the environment today.");
}

#[test]
fn given_windows_style_crlf_newlines_when_sanitizing_then_merges_word() {
    let input = "The operating sys-\r\ntem is ready.";
    let result = sanitize_extracted_text(input);
    assert_eq!(result, "The operating system is ready.");
}

#[test]
fn given_soft_wrapped_lines_when_sanitizing_then_preserves_single_newlines() {
    let input = "This is line one\nThis is line two";
    let result = sanitize_extracted_text(input);
    assert_eq!(result, "This is line one\nThis is line two");
}

#[test]
fn given_messy_blank_lines_with_tabs_when_sanitizing_then_normalizes_to_clean_paragraph_break() {
    let input = "Paragraph One\n  \t  \nParagraph Two";
    let result = sanitize_extracted_text(input);
    assert_eq!(result, "Paragraph One\n\nParagraph Two");
}

#[test]
fn given_unicode_characters_split_by_hyphen_when_sanitizing_then_merges_correctly() {
    let input = "Thank you for your coöper- \n ation.";
    let result = sanitize_extracted_text(input);
    assert_eq!(result, "Thank you for your coöperation.");
}

#[test]
fn given_math_equation_minus_sign_at_end_of_line_when_sanitizing_then_preserves_space() {
    let input = "The formula is x - \n y";
    let result = sanitize_extracted_text(input);

    assert_eq!(result, "The formula is x -\ny");
}
