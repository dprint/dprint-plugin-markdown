use super::super::configuration::Configuration;

pub struct Context<'a> {
    pub file_text: &'a str,
    pub configuration: &'a Configuration,
    pub indent_level: u32,
    pub is_in_list_count: u32,
    pub format_code_block_text: Box<dyn Fn(&str, &str) -> Result<String, String>>,
}

impl<'a> Context<'a> {
    pub fn new(
        file_text: &'a str,
        configuration: &'a Configuration,
        format_code_block_text: Box<dyn Fn(&str, &str) -> Result<String, String>>
    ) -> Context<'a> {
        Context {
            file_text,
            configuration,
            indent_level: 0,
            is_in_list_count: 0,
            format_code_block_text,
        }
    }

    pub fn is_in_list(&self) -> bool {
        self.is_in_list_count > 0
    }

    pub fn format_text(&self, tag: &str, text: &str) -> Result<String, String> {
        (self.format_code_block_text)(tag, text)
    }

    pub fn get_new_lines_in_range(&self, start: usize, end: usize) -> u32 {
        if end < start {
            return 0;
        } // ignore

        let file_bytes = self.file_text.as_bytes();
        let mut count = 0;
        for byte in &file_bytes[start..end] {
            if byte == &('\n' as u8) {
                count += 1;
            }
        }
        count
    }

    pub fn get_indent_level_at_pos(&self, pos: usize) -> u32 {
        let file_bytes = self.file_text.as_bytes();
        let mut count = 0;

        for byte in file_bytes[0..pos].iter().rev() {
            // This is ok because we are just investigating whitespace chars
            // which I believe are only 1 byte.
            let character = *byte as char;

            if character == '\n' {
                break;
            }

            if character == '\t' {
                count += 4;
            } else if character.is_whitespace() {
                count += 1;
            } else {
                // todo: unexpected... I guess break?
                break;
            }
        }

        const INDENT_WIDTH: usize = 1;
        (count as f64 / INDENT_WIDTH as f64).round() as u32
    }
}
