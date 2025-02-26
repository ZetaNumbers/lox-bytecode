use std::cell::RefCell;

use crate::chunk::*;
use crate::scanner::*;
use crate::token::*;
use crate::value::*;
use crate::*;

pub struct Compiler<'a> {
    parser: Parser,
    scanner: Scanner,
    chunk: &'a mut Chunk,
}

#[derive(Default)]
pub struct Parser {
    current: Token,
    previous: Token,
    had_error: RefCell<bool>,
    panic_mode: RefCell<bool>,
}

struct ParseRule {
    prefix: Option<fn(&mut Compiler)>,
    infix: Option<fn(&mut Compiler)>,
    precedence: Precedence,
}

#[derive(PartialEq, PartialOrd, Copy, Clone)]
enum Precedence {
    None = 0,
    Assignment, // =
    Or,         // or
    And,        // and
    Equality,   // == !=
    Comparison, // < > <= >=
    Term,       // + -
    Factor,     // * /
    Unary,      // ! -
    Call,       // . ()
    Primary,
}

impl From<usize> for Precedence {
    fn from(v: usize) -> Self {
        match v {
            0 => Precedence::None,
            1 => Precedence::Assignment,
            2 => Precedence::Or,
            3 => Precedence::And,
            4 => Precedence::Equality,
            5 => Precedence::Comparison,
            6 => Precedence::Term,
            7 => Precedence::Factor,
            8 => Precedence::Unary,
            9 => Precedence::Call,
            10 => Precedence::Primary,
            v => panic!("cannot convert {v} into Precedence"),
        }
    }
}

impl Precedence {
    fn next(self) -> Self {
        if self == Precedence::Primary {
            panic!("no next after Primary");
        }
        let p = self as usize;
        (p + 1).into()
    }

    fn previous(self) -> Self {
        if self == Precedence::None {
            panic!("no previous before None");
        }
        let p = self as usize;
        (p - 1).into()
    }
}

impl<'a> Compiler<'a> {
    pub fn new(chunk: &'a mut Chunk) -> Self {
        Self {
            parser: Parser::default(),
            scanner: Scanner::new(&"".to_string()),
            chunk,
        }
    }

    pub fn compile(&mut self, source: &String) -> Result<(), InterpretResult> {
        self.scanner = Scanner::new(source);
        self.advance();

        /*
        self.expression()?;
        */

        self.consume(TokenType::Eof, "Expect end of expression.");

        self.end_compiler();

        if *self.parser.had_error.borrow() {
            Err(InterpretResult::CompileError)
        } else {
            Ok(())
        }
    }

    fn advance(&mut self) {
        self.parser.previous = self.parser.current.clone();

        loop {
            self.parser.current = self.scanner.scan_token();
            if self.parser.current.ttype != TokenType::Error {
                break;
            }

            let message = self.parser.current.lexeme.as_str();
            self.error_at_current(message);
        }
    }

    fn consume(&mut self, ttype: TokenType, message: &str) {
        if self.parser.current.ttype == ttype {
            self.advance();
            return;
        }

        self.error_at_current(message);
    }

    fn emit_byte(&mut self, byte: u8) {
        self.chunk.write(byte, self.parser.previous.line);
    }

    fn emit_bytes(&mut self, byte1: OpCode, byte2: u8) {
        self.emit_byte(byte1.into());
        self.emit_byte(byte2);
    }

    fn emit_return(&mut self) {
        self.emit_byte(OpCode::Return.into());
    }

    fn make_constant(&mut self, value: Value) -> u8 {
        if let Some(constant) = self.chunk.add_constant(value) {
            constant
        } else {
            self.error("Too many constants in one chunk.");
            0
        }
    }

    fn emit_constant(&mut self, value: Value) {
        let constant = self.make_constant(value);
        self.emit_bytes(OpCode::Constant, constant);
    }

    fn end_compiler(&mut self) {
        self.emit_return();
    }

    fn binary(&mut self) {
        let operator_type = self.parser.previous.ttype;
        let rule = self.get_rule(operator_type);

        self.parse_precedence(rule.precedence.next());

        match operator_type {
            TokenType::Plus => self.emit_byte(OpCode::Add.into()),
            TokenType::Minus => self.emit_byte(OpCode::Subtract.into()),
            TokenType::Star => self.emit_byte(OpCode::Multiply.into()),
            TokenType::Slash => self.emit_byte(OpCode::Divide.into()),
            _ => todo!(),
        }
    }

    fn grouping(&mut self) {
        self.expression();
        self.consume(TokenType::RightParen, "Expect ')' after expression.");
    }

    fn number(&mut self) {
        let value = self.parser.previous.lexeme.parse::<Value>().unwrap();
        self.emit_constant(value);
    }

    fn unary(&mut self) {
        let operator_type = self.parser.previous.ttype;

        self.parse_precedence(Precedence::Unary);

        if operator_type == TokenType::Minus {
            self.emit_byte(OpCode::Negate.into())
        } else {
            unimplemented!("nope");
        }
    }

    fn parse_precedence(&self, precedence: Precedence) {}

    fn get_rule(&self, ttype: TokenType) -> ParseRule {
        match ttype {
            TokenType::LeftParen => ParseRule {
                prefix: Some(|c| c.grouping()),
                infix: None,
                precedence: Precedence::None,
            },
            _ => ParseRule {
                prefix: None,
                infix: None,
                precedence: Precedence::None,
            },
        }
    }

    fn expression(&mut self) {
        self.parse_precedence(Precedence::Assignment);
    }

    fn error_at_current(&self, message: &str) {
        self.error_at(&self.parser.current, message);
    }

    fn error(&self, message: &str) {
        self.error_at(&self.parser.previous, message);
    }

    fn error_at(&self, token: &Token, message: &str) {
        if *self.parser.panic_mode.borrow() {
            return;
        }

        self.parser.panic_mode.replace(true);

        eprint!("[line {}] Error", token.line);

        if token.ttype == TokenType::Eof {
            eprint!(" at end");
        } else if token.ttype == TokenType::Error {
            // ignore
        } else {
            eprint!(" at '{}'", token.lexeme);
        }

        eprintln!(": {message}");
        self.parser.had_error.replace(true);
    }
}
