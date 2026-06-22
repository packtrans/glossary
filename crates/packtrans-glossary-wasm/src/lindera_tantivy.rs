//! Minimal Lindera tokenizer adapter for Tantivy (from `lindera-tantivy`, `from_segmenter` only).

use lindera::segmenter::Segmenter;
use lindera::tokenizer::Tokenizer as LinderaInnerTokenizer;
use lindera::token::Token as LinderaToken;
use tantivy_tokenizer_api::{Token, TokenStream, Tokenizer};

#[derive(Clone)]
pub struct LinderaTokenizer {
    tokenizer: LinderaInnerTokenizer,
    token: Token,
}

impl LinderaTokenizer {
    pub fn from_segmenter(segmenter: Segmenter) -> Self {
        Self {
            tokenizer: LinderaInnerTokenizer::new(segmenter),
            token: Token::default(),
        }
    }
}

impl Tokenizer for LinderaTokenizer {
    type TokenStream<'a> = LinderaTokenStream<'a>;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> LinderaTokenStream<'a> {
        self.token.reset();
        LinderaTokenStream {
            tokens: self.tokenizer.tokenize(text).unwrap(),
            token: &mut self.token,
            current_index: 0,
        }
    }
}

pub struct LinderaTokenStream<'a> {
    tokens: Vec<LinderaToken<'a>>,
    token: &'a mut Token,
    current_index: usize,
}

impl TokenStream for LinderaTokenStream<'_> {
    fn advance(&mut self) -> bool {
        if self.current_index >= self.tokens.len() {
            return false;
        }

        let token = &self.tokens[self.current_index];
        self.token.text = token.surface.to_string();
        self.token.offset_from = token.byte_start;
        self.token.offset_to = token.byte_end;
        self.token.position = token.position;
        self.token.position_length = token.position_length;
        self.current_index += 1;
        true
    }

    fn token(&self) -> &Token {
        self.token
    }

    fn token_mut(&mut self) -> &mut Token {
        self.token
    }
}
