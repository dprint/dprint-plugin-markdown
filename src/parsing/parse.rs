use dprint_core::*;
use dprint_core::{conditions::*, parser_helpers::*, condition_resolvers};
use super::common::*;
use super::parser_types::*;
use super::utils;

pub fn parse_node(node: &Node, context: &mut Context) -> PrintItems {
    // println!("Text: {:?}", node.text(context));
    match node {
        Node::SourceFile(node) => parse_source_file(node, context),
        Node::Heading(node) => parse_heading(node, context),
        Node::Paragraph(node) => parse_paragraph(node, context),
        Node::BlockQuote(node) => parse_block_quote(node, context),
        Node::CodeBlock(node) => parse_code_block(node, context),
        Node::Code(node) => parse_code(node, context),
        Node::Text(node) => parse_text(node, context),
        Node::TextDecoration(node) => parse_text_decoration(node, context),
        Node::Html(node) => parse_html(node, context),
        Node::FootnoteReference(node) => parse_footnote_reference(node, context),
        Node::FootnoteDefinition(node) => parse_footnote_definition(node, context),
        Node::InlineLink(node) => parse_inline_link(node, context),
        Node::ReferenceLink(node) => parse_reference_link(node, context),
        Node::ShortcutLink(node) => parse_shortcut_link(node, context),
        Node::AutoLink(node) => parse_auto_link(node, context),
        Node::LinkReference(node) => parse_link_reference(node, context),
        Node::InlineImage(node) => parse_inline_image(node, context),
        Node::ReferenceImage(node) => parse_reference_image(node, context),
        Node::List(node) => parse_list(node, context),
        Node::Item(node) => parse_item(node, context),
        Node::TaskListMarker(node) => parse_task_list_marker(node, context),
        Node::HorizontalRule(node) => parse_horizontal_rule(node, context),
        Node::SoftBreak(_) => PrintItems::new(),
        Node::HardBreak(_) => Signal::NewLine.into(),
        Node::Table(node) => parse_table(node, context),
        Node::TableHead(_) => unreachable!(),
        Node::TableRow(_) => unreachable!(),
        Node::TableCell(node) => parse_table_cell(node, context),
        Node::NotImplemented(_) => parse_raw_string(node.text(context)),
    }
}

fn parse_source_file(source_file: &SourceFile, context: &mut Context) -> PrintItems {
    let mut items = PrintItems::new();

    if let Some(yaml_header) = &source_file.yaml_header {
        items.extend(parser_helpers::parse_raw_string(&yaml_header.text));

        if source_file.children.len() > 0 {
            items.push_signal(Signal::NewLine);
            items.push_signal(Signal::NewLine);
        }
    }

    items.extend(parse_nodes(&source_file.children, context));

    items.push_condition(if_true(
        "endOfFileNewLine",
        |context| Some(context.writer_info.column_number > 0 || context.writer_info.line_number > 0),
        Signal::NewLine.into()
    ));

    items
}

fn parse_nodes(nodes: &Vec<Node>, context: &mut Context) -> PrintItems {
    let mut items = PrintItems::new();
    let mut last_node: Option<&Node> = None;

    for node in nodes {
        // todo: this area needs to be thought out more
        let is_current_soft_break = match node { Node::SoftBreak(_) => true, _=> false, };
        if let Some(last_node) = last_node {
            match last_node {
                Node::Heading(_) | Node::Paragraph(_) | Node::CodeBlock(_) | Node::FootnoteDefinition(_) | Node::HorizontalRule(_) | Node::List(_) => {
                    items.push_signal(Signal::NewLine);
                    items.push_signal(Signal::NewLine);
                },
                Node::Text(_) | Node::Html(_) => {
                    let between_range = (last_node.range().end, node.range().start);
                    let new_line_count = context.get_new_lines_in_range(between_range.0, between_range.1);
                    if new_line_count > 0 {
                        items.push_signal(Signal::NewLine);
                        if new_line_count > 1 {
                            items.push_signal(Signal::NewLine);
                        }
                    } else if between_range.0 < between_range.1 {
                        items.push_signal(Signal::SpaceOrNewLine)
                    }
                },
                Node::Code(_) | Node::SoftBreak(_) | Node::TextDecoration(_) | Node::FootnoteReference(_) => {
                    let needs_space = if let Node::Text(text) = node {
                        !text.starts_with_punctuation() || text.has_preceeding_space(&context.file_text)
                    } else {
                        true
                    };

                    if needs_space && !is_current_soft_break {
                        if node.starts_with_list_char() {
                            items.push_str(" ");
                        } else {
                            items.push_signal(Signal::SpaceOrNewLine);
                        }
                    }
                },
                Node::InlineLink(_) | Node::ReferenceLink(_) | Node::ShortcutLink(_) | Node::AutoLink(_) => {
                    let needs_space = if let Node::Text(text) = node {
                        !text.starts_with_punctuation() || text.has_preceeding_space(&context.file_text)
                    } else {
                        false
                    };

                    if needs_space {
                        if node.starts_with_list_char() {
                            items.push_str(" ");
                        } else {
                            items.push_signal(Signal::SpaceOrNewLine);
                        }
                    }
                },
                Node::LinkReference(_) => {
                    let needs_newline = if let Node::LinkReference(_) = node { true } else { false };
                    if needs_newline {
                        items.push_signal(Signal::NewLine);
                    }
                }
                Node::TaskListMarker(_) => {
                    items.push_str(" ");
                },
                _ => {},
            }
        }

        items.extend(parse_node(node, context));
        last_node = Some(node);
    }

    items
}

fn parse_heading(heading: &Heading, context: &mut Context) -> PrintItems {
    let mut items = PrintItems::new();

    items.push_str(&format!("{} ", "#".repeat(heading.level as usize)));
    items.extend(with_no_new_lines(parse_nodes(&heading.children, context)));

    items
}

fn parse_paragraph(paragraph: &Paragraph, context: &mut Context) -> PrintItems {
    parse_nodes(&paragraph.children, context)
}

fn parse_block_quote(block_quote: &BlockQuote, context: &mut Context) -> PrintItems {
    let mut items = PrintItems::new();
    items.push_str("> ");

    // add a > for any string that is on the start of a line
    for print_item in parse_nodes(&block_quote.children, context).iter() {
        match print_item {
            PrintItem::String(text) => {
                items.push_condition(if_true_or(
                    "isStartOfLine",
                    |context| Some(context.writer_info.is_start_of_line()),
                    {
                        let mut items = PrintItems::new();
                        items.push_str("> ");
                        items.push_item(PrintItem::String(text.clone()));
                        items
                    },
                    {
                        let mut items = PrintItems::new();
                        items.push_item(PrintItem::String(text));
                        items
                    },
                ));
            },
            _ => items.push_item(print_item),
        }
    }

    items
}

fn parse_code_block(code_block: &CodeBlock, _: &mut Context) -> PrintItems {
    let mut items = PrintItems::new();

    items.push_str("```");
    if let Some(tag) = &code_block.tag {
        items.push_str(tag);
    }
    items.push_signal(Signal::NewLine);

    // body
    items.extend(parser_helpers::parse_string(&code_block.code.trim()));

    // footer
    items.push_signal(Signal::NewLine);
    items.push_str("```");

    items
}

fn parse_code(code: &Code, _: &mut Context) -> PrintItems {
    format!("`{}`", code.code.trim()).into()
}

fn parse_text(text: &Text, _: &mut Context) -> PrintItems {
    let mut text_builder = TextBuilder::new();

    for c in text.text.chars() {
        if c.is_whitespace() {
            text_builder.space_or_new_line();
        } else {
            text_builder.add_char(c);
        }
    }

    return text_builder.build();

    struct TextBuilder {
        items: PrintItems,
        was_last_whitespace: bool,
        current_word: Option<String>,
    }

    impl TextBuilder {
        pub fn new() -> TextBuilder {
            TextBuilder {
                items: PrintItems::new(),
                was_last_whitespace: false,
                current_word: None,
            }
        }

        pub fn build(mut self) -> PrintItems {
            self.flush_current_word();
            self.items
        }

        pub fn space_or_new_line(&mut self) {
            if self.items.is_empty() && self.current_word.is_none() { return; }
            if self.was_last_whitespace { return; }

            self.flush_current_word();

            self.was_last_whitespace = true;
        }

        pub fn add_char(&mut self, character: char) {
            if self.was_last_whitespace {
                if utils::is_list_char(character) {
                    self.items.push_str(" ");
                } else {
                    self.items.push_signal(Signal::SpaceOrNewLine);
                }
                self.was_last_whitespace = false;
            }

            if let Some(current_word) = self.current_word.as_mut() {
                current_word.push(character);
            } else {
                let mut text = String::new();
                text.push(character);
                self.current_word = Some(text);
            }
        }

        fn flush_current_word(&mut self) {
            if let Some(current_word) = self.current_word.take() {
                self.items.push_str(&current_word);
            }
        }
    }
}

fn parse_text_decoration(text: &TextDecoration, context: &mut Context) -> PrintItems {
    let mut items = PrintItems::new();
    let decoration_text = match &text.kind {
        TextDecorationKind::Emphasis => "_", // todo: config for * instead
        TextDecorationKind::Strong => "**", // todo: config for __ instead
        TextDecorationKind::Strikethrough => "~~",
    };

    items.push_str(&decoration_text);
    items.extend(parse_nodes(&text.children, context));
    items.push_str(&decoration_text);

    items
}

fn parse_html(html: &Html, _: &mut Context) -> PrintItems {
    let mut items = PrintItems::new();
    items.push_str(html.text.trim_end());
    items
}

fn parse_footnote_reference(footnote_reference: &FootnoteReference, _: &mut Context) -> PrintItems {
    let mut items = PrintItems::new();
    items.push_str(&format!("[^{}]", footnote_reference.name.trim()));
    items
}

fn parse_footnote_definition(footnote_definition: &FootnoteDefinition, context: &mut Context) -> PrintItems {
    let mut items = PrintItems::new();
    items.push_str(&format!("[^{}]: ", footnote_definition.name.trim()));
    items.extend(parse_nodes(&footnote_definition.children, context));
    items
}

fn parse_inline_link(link: &InlineLink, context: &mut Context) -> PrintItems {
    let mut items = PrintItems::new();
    items.push_str("[");
    items.extend(parse_nodes(&link.children, context));
    items.push_str("]");
    items.push_str("(");
    items.push_str(&link.url.trim());
    if let Some(title) = &link.title {
        items.push_str(&format!(" \"{}\"", title.trim()));
    }
    items.push_str(")");
    items
}

fn parse_reference_link(link: &ReferenceLink, context: &mut Context) -> PrintItems {
    let mut items = PrintItems::new();
    items.push_str("[");
    items.extend(parse_nodes(&link.children, context));
    items.push_str("]");
    items.push_str(&format!("[{}]", link.reference.trim()));
    items
}

fn parse_shortcut_link(link: &ShortcutLink, context: &mut Context) -> PrintItems {
    let mut items = PrintItems::new();
    items.push_str("[");
    items.extend(parse_nodes(&link.children, context));
    items.push_str("]");
    items
}

fn parse_auto_link(link: &AutoLink, context: &mut Context) -> PrintItems {
    let mut items = PrintItems::new();
    items.push_str("<");
    items.extend(parse_nodes(&link.children, context));
    items.push_str(">");
    items
}

fn parse_link_reference(link_ref: &LinkReference, _: &mut Context) -> PrintItems {
    let mut items = PrintItems::new();
    items.push_str(&format!("[{}]: ", link_ref.name.trim()));
    items.push_str(&link_ref.link.trim());
    if let Some(title) = &link_ref.title {
        items.push_str(&format!(" \"{}\"", title.trim()));
    }
    items
}

fn parse_inline_image(image: &InlineImage, _: &mut Context) -> PrintItems {
    let mut items = PrintItems::new();
    items.push_str(&format!("![{}]", image.text.trim()));
    items.push_str("(");
    items.push_str(&image.url.trim());
    if let Some(title) = &image.title {
        items.push_str(&format!(" \"{}\"", title.trim()));
    }
    items.push_str(")");
    items
}

fn parse_reference_image(image: &ReferenceImage, _: &mut Context) -> PrintItems {
    let mut items = PrintItems::new();
    items.push_str(&format!("![{}]", image.text.trim()));
    items.push_str(&format!("[{}]", image.reference.trim()));
    items
}

fn parse_list(list: &List, context: &mut Context) -> PrintItems {
    let mut items = PrintItems::new();
    for (index, child) in list.children.iter().enumerate() {
        if index > 0 { items.push_signal(Signal::NewLine); }
        let prefix_text = if let Some(start_index) = list.start_index {
            format!("{}.", start_index + index as u64)
        } else {
            String::from("-")
        };
        items.push_str(&prefix_text);
        let after_child = Info::new("afterChild");
        items.push_condition(if_true(
            "spaceIfHasChild",
            move |context| Some(!condition_resolvers::is_at_same_position(context, &after_child)?),
            " ".into()
        ));
        items.extend(with_indent_times(parse_node(child, context), (prefix_text.chars().count() + 1) as u32));
        items.push_info(after_child);
    }
    items
}

fn parse_item(item: &Item, context: &mut Context) -> PrintItems {
    parse_nodes(&item.children, context)
}

fn parse_task_list_marker(marker: &TaskListMarker, _: &mut Context) -> PrintItems {
    if marker.is_checked {
        "[x]".into()
    } else {
        "[ ]".into()
    }
}

fn parse_horizontal_rule(_: &HorizontalRule, _: &mut Context) -> PrintItems {
    "---".into()
}

fn parse_table(table: &Table, context: &mut Context) -> PrintItems {
    let header = table.header.cells.iter().map(|cell| get_cell_items_and_width(cell, context)).collect::<Vec<_>>();
    let rows = table.rows.iter().map(|row| row.cells.iter().map(|cell| get_cell_items_and_width(cell, context)).collect::<Vec<_>>()).collect::<Vec<_>>();
    let column_widths = get_column_widths(&header, &rows);
    let mut items = PrintItems::new();

    items.extend(get_row_items(header, &column_widths, &table.column_alignment));
    items.push_signal(Signal::NewLine);
    items.extend(get_divider_row(&column_widths, &table.column_alignment));

    for row in rows {
        items.push_signal(Signal::NewLine);
        items.extend(get_row_items(row, &column_widths, &table.column_alignment));
    }

    return items;

    fn get_divider_row(column_widths: &Vec<usize>, column_alignments: &Vec<ColumnAlignment>) -> PrintItems {
        let mut items = PrintItems::new();
        for (i, column_width) in column_widths.iter().enumerate() {
            let column_alignment = column_alignments.get(i).map(|x| *x).unwrap_or(ColumnAlignment::None);
            if i == 0 {
                items.push_str("| ");
            } else {
                items.push_str(" ");
            }

            let has_left_colon = column_alignment == ColumnAlignment::Left || column_alignment == ColumnAlignment::Center;
            let has_right_colon = column_alignment == ColumnAlignment::Right || column_alignment == ColumnAlignment::Center;
            let dashes_count = column_width - (if has_left_colon { 1 } else { 0 }) - (if has_right_colon { 1 } else { 0 });

            if has_left_colon { items.push_str(":"); }
            items.push_str(&"-".repeat(dashes_count));
            if has_right_colon { items.push_str(":"); }

            items.push_str(" |");
        }

        parser_helpers::with_no_new_lines(items)
    }

    fn get_row_items(row_cells: Vec<(PrintItems, usize)>, column_widths: &Vec<usize>, column_alignments: &Vec<ColumnAlignment>) -> PrintItems {
        let mut items = PrintItems::new();
        for (i, (cell_items, cell_width)) in row_cells.into_iter().enumerate() {
            let column_alignment = column_alignments.get(i).map(|x| *x).unwrap_or(ColumnAlignment::None);
            let column_max_width = *column_widths.get(i).unwrap();
            let difference = column_max_width - cell_width;
            if i == 0 {
                items.push_str("| ")
            } else {
                items.push_str(" ");
            }

            if difference > 0 {
                match column_alignment {
                    ColumnAlignment::None | ColumnAlignment::Left => {},
                    ColumnAlignment::Center => {
                        if difference > 1 {
                            items.push_str(&" ".repeat((difference as f32 / 2_f32).floor() as usize))
                        }
                    },
                    ColumnAlignment::Right => {
                        items.push_str(&" ".repeat(difference));
                    },
                }
            }

            items.extend(cell_items);

            if difference > 0 {
                match column_alignment {
                    ColumnAlignment::None | ColumnAlignment::Left => {
                        items.push_str(&" ".repeat(difference));
                    },
                    ColumnAlignment::Center => {
                        if difference > 1 {
                            items.push_str(&" ".repeat((difference as f32 / 2_f32).ceil() as usize))
                        }
                    },
                    ColumnAlignment::Right => {},
                }
            }

            items.push_str(" |");
        }

        parser_helpers::with_no_new_lines(items)
    }

    fn get_column_widths(header: &Vec<(PrintItems, usize)>, rows: &Vec<Vec<(PrintItems, usize)>>) -> Vec<usize> {
        let mut column_widths = Vec::new();
        for i in 0.. {
            let mut had_column = false;
            let mut max_width = 0;
            if let Some((_, width)) = header.get(i) {
                max_width = *width;
                had_column = true;
            }

            for row in rows.iter() {
                if let Some((_, width)) = row.get(i) {
                    max_width = std::cmp::max(max_width, *width);
                    had_column = true;
                }
            }

            if had_column {
                column_widths.push(max_width);
            } else {
                break;
            }
        }
        column_widths
    }

    fn get_cell_items_and_width(cell: &TableCell, context: &mut Context) -> (PrintItems, usize) {
        let items = parse_table_cell(cell, context);
        let (items, cloned_items) = clone_items(items);
        let width = measure_single_line_width(cloned_items);
        (items, width)
    }
}

fn parse_table_cell(table_cell: &TableCell, context: &mut Context) -> PrintItems {
    parse_nodes(&table_cell.children, context)
}

fn clone_items(items: PrintItems) -> (PrintItems, PrintItems) {
    // todo: something in the core library? This is weird
    let rc_path = items.into_rc_path();
    let mut items1 = PrintItems::new();
    let mut items2 = PrintItems::new();
    items1.push_optional_path(rc_path.clone());
    items2.push_optional_path(rc_path);
    (items1, items2)
}

fn measure_single_line_width(items: PrintItems) -> usize {
    // this doesn't seem ideal, but good enough for now
    print(parser_helpers::with_no_new_lines(items), PrintOptions {
        indent_width: 0,
        max_width: std::u32::MAX,
        use_tabs: false,
        new_line_text: "",
    }).chars().count()
}
