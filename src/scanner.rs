use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token<'a> {
    Ident(&'a str),
    Keyword(Keyword),
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Equals,
    Semicolon,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    Extern,
    Static,
    Other,
}

pub struct Scanner<'a> {
    input: &'a str,
}

impl<'a> Scanner<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input }
    }

    fn consume_until(&mut self, pat: &str) {
        let end_index = self
            .input
            .find(pat)
            .map_or(self.input.len(), |n| n + pat.len());

        self.input = &self.input[end_index..];
    }

    fn consume_ident(&mut self) -> &'a str {
        let end_index = self
            .input
            .find(|c| !is_ident(c))
            .unwrap_or(self.input.len());

        let ident = &self.input[..end_index];
        self.input = &self.input[end_index..];

        ident
    }

    fn consume_one(&mut self) {
        self.input = &self.input[1..];
    }
}

impl<'a> Iterator for Scanner<'a> {
    type Item = Token<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            self.input = self.input.trim_start();

            let token = match self.input.chars().next()? {
                '/' if self.input.starts_with("//") => {
                    self.consume_until("\n");
                    continue;
                }
                '/' if self.input.starts_with("/*") => {
                    self.consume_until("*/");
                    continue;
                }
                '#' => {
                    self.consume_until("\n");
                    continue;
                }

                '(' => {
                    self.consume_one();
                    Token::LParen
                }
                ')' => {
                    self.consume_one();
                    Token::RParen
                }
                '[' => {
                    self.consume_one();
                    Token::LBracket
                }
                ']' => {
                    self.consume_one();
                    Token::RBracket
                }
                '{' => {
                    self.consume_one();
                    Token::LBrace
                }
                '}' => {
                    self.consume_one();
                    Token::RBrace
                }
                '=' if !self.input.starts_with("==") => {
                    self.consume_one();
                    Token::Equals
                }
                ';' => {
                    self.consume_one();
                    Token::Semicolon
                }

                '"' => {
                    let mut escaped = false;
                    for (i, c) in self.input.char_indices().skip(1) {
                        if c == '"' && !escaped {
                            self.input = &self.input[i + 1..];
                            break;
                        }
                        escaped = c == '\\';
                    }
                    Token::Other
                }
                '\'' => {
                    let mut escaped = false;
                    for (i, c) in self.input.char_indices().skip(1) {
                        if c == '\'' && !escaped {
                            self.input = &self.input[i + 1..];
                            break;
                        }
                        escaped = c == '\\';
                    }
                    Token::Other
                }

                c if c.is_numeric() => {
                    self.consume_ident();
                    Token::Other
                }
                c if is_ident(c) => {
                    let ident = self.consume_ident();
                    match Keyword::from_str(ident) {
                        Ok(keyword) => Token::Keyword(keyword),
                        Err(()) => Token::Ident(ident),
                    }
                }

                c => {
                    self.input = &self.input[c.len_utf8()..];
                    Token::Other
                }
            };

            return Some(token);
        }
    }
}

fn is_ident(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

impl FromStr for Keyword {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "extern" => Ok(Self::Extern),
            "static" => Ok(Self::Static),
            "auto" | "break" | "case" | "char" | "const" | "continue" | "default" | "do"
            | "double" | "else" | "enum" | "float" | "for" | "goto" | "if" | "inline" | "int"
            | "long" | "register" | "restrict" | "return" | "short" | "signed" | "sizeof"
            | "switch" | "typedef" | "union" | "unsigned" | "void" | "volatile" | "while"
            | "_Bool" | "_Complex" | "_Imaginary" => Ok(Self::Other),
            _ => Err(()),
        }
    }
}
