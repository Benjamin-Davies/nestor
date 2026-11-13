use std::fmt;

use pulldown_cmark::{CodeBlockKind, CowStr, Event, Options, Tag, TagEnd};

#[derive(Default)]
pub struct Markdown {
    events: Vec<Event<'static>>,
}

pub struct InlineBuilder<'a> {
    inner: &'a mut Markdown,
}

impl Markdown {
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn push_rule(&mut self) {
        self.events.push(Event::Rule);
    }

    pub fn push_code_block(&mut self, code: impl Into<CowStr<'static>>) {
        self.events
            .push(Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(
                "c".into(),
            ))));
        self.events.push(Event::Text(code.into()));
        self.events.push(Event::End(TagEnd::CodeBlock));
    }

    pub fn build_paragraph(&mut self) -> InlineBuilder<'_> {
        self.events.push(Event::Start(Tag::Paragraph));
        InlineBuilder { inner: self }
    }

    pub fn append(&mut self, mut other: Markdown) {
        self.events.append(&mut other.events);
    }
}

impl<'a> InlineBuilder<'a> {
    pub fn push_text(&mut self, text: impl Into<CowStr<'static>>) -> &mut Self {
        self.inner.events.push(Event::Text(text.into()));
        self
    }

    pub fn push_code(&mut self, text: impl Into<CowStr<'static>>) -> &mut Self {
        self.inner.events.push(Event::Code(text.into()));
        self
    }
}

impl<'a> Drop for InlineBuilder<'a> {
    fn drop(&mut self) {
        self.inner.events.push(Event::End(TagEnd::Paragraph));
    }
}

impl fmt::Display for Markdown {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        pulldown_cmark_to_cmark::cmark(&self.events, f, Options::empty()).unwrap();
        Ok(())
    }
}

impl fmt::Debug for Markdown {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "\"\"\"\n{self}\n\"\"\"")
    }
}
