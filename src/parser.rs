use super::lexer::{Token, TokenKind};
use std::fmt;

#[derive(Debug)]
pub struct Parser {
    tokens: Vec<Token>,
    index: usize,
    line: usize,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Instruction(TokenKind), // Conditions only
    Op(TokenKind),          // Operations only
    Expression,
    Integer(u16),
    Byte(u8),
    String(String),
    Label(String),
    Register(TokenKind), // Registers only
    Unary(TokenKind),    // Unary operators only (+ or -)
    // Binary,
    // Grouping,
    Directive(TokenKind),
}

#[derive(Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub exprs: Vec<Expr>,
    pub line: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            index: 0,
            line: 1,
        }
    }

    fn peek(&self) -> Option<&Token> {
        if self.index != self.tokens.len() {
            self.tokens.get(self.index)
        } else {
            None
        }
    }

    fn next(&mut self) -> Option<&Token> {
        match self.tokens.get(self.index) {
            Some(t) if self.index != self.tokens.len() => {
                self.index += 1;
                self.line = t.2;
                Some(t)
            }
            _ => None,
        }
    }

    /*
     *[X] statement   = instruction | directive | label
     *
     *[X] instruction = op | op "?" CONDITION
     *[X] op          = OPCODE | OPCODE (REGISTER | expression) | OPCODE (REGISTER | expression) "->" (REGISTER | expression)
     *[/] expression  = INTEGER | LABEL | unary | binary | grouping
     *[X] unary       = OPERATOR_UNARY expression
     *[ ] binary      = expression OPERATOR_BINARY expression
     *[ ] grouping    = "(" expression ")"
     *
     *[X] directive   = DIRECTIVE (expression | BYTE | STRING)*
     *
     *[X] label       = LABEL ":"
     */

    pub fn parse(&mut self) -> Result<Vec<Expr>, String> {
        let mut output: Vec<Expr> = Vec::new();

        loop {
            match self.parse_one_statement() {
                Ok(Some(statement)) => output.push(statement),
                Ok(None) => break,
                Err(e) => return Err(format!("Error on line {}:\n  {}", self.line, e)),
            };
        }

        Ok(output)
    }

    pub fn parse_one_statement(&mut self) -> Result<Option<Expr>, String> {
        if let Some(i) = self.instruction()? {
            Ok(Some(i))
        } else if let Some(d) = self.directive()? {
            Ok(Some(d))
        } else if let Some(l) = self.label()? {
            Ok(Some(l))
        } else if let Some(t) = self.peek() {
            Err(format!("Unexpected token '{}'", t))
        } else {
            Ok(None)
        }
    }

    fn instruction(&mut self) -> Result<Option<Expr>, String> {
        // Ensure that there's an operation to be read in
        let op = match self.op()? {
            Some(op) => op,
            None => return Ok(None),
        };

        // place operation into new instruction struct
        let mut instruction = Expr {
            kind: ExprKind::Instruction(TokenKind::None),
            exprs: vec![],
            line: op.line,
        };
        instruction.exprs.push(op);

        // Peek for next token
        let peek = match self.peek() {
            Some(t) => t,
            None => return Ok(Some(instruction)),
        };

        // Check if there's a '?' for the conditional
        if !matches!(peek.0, TokenKind::QuestionMark) {
            return Ok(Some(instruction)); // If there's none, just return the op with no condition as an instruction
        }

        let current_line = peek.2;

        self.next(); // Consume the '?'

        // Get the next word for the condition if it exists
        let peek = match self.peek() {
            Some(t) => t,
            None => return Err(format!("No condition after '?' on line {}", current_line)),
        };

        // Check that the peeked token is in fact a condition, and if so, set that to op's cond
        match peek.0 {
            TokenKind::Eq
            | TokenKind::Neq
            | TokenKind::Lt
            | TokenKind::Gte
            | TokenKind::Gt
            | TokenKind::Lte
            | TokenKind::Cr
            | TokenKind::Ncr => instruction.kind = ExprKind::Instruction(peek.0.to_owned()),
            _ => return Err("No condition after '?'".to_owned()),
        };

        // Consume conditional token
        self.next();

        Ok(Some(instruction))
    }

    fn op(&mut self) -> Result<Option<Expr>, String> {
        let op_token = match self.peek() {
            Some(t) => t,
            None => return Ok(None),
        };

        let mut op = match op_token.0 {
            TokenKind::Add
            | TokenKind::Sub
            | TokenKind::Mul
            | TokenKind::Div
            | TokenKind::Mov
            | TokenKind::Inc
            | TokenKind::Dec
            | TokenKind::Cmp
            | TokenKind::Ldr
            | TokenKind::Str
            | TokenKind::Ldx
            | TokenKind::Stx
            | TokenKind::Lsl
            | TokenKind::Lsr
            | TokenKind::Ssp
            | TokenKind::Gsp
            | TokenKind::Or
            | TokenKind::And
            | TokenKind::Not
            | TokenKind::Xor
            | TokenKind::Flg
            | TokenKind::Push
            | TokenKind::Pshx
            | TokenKind::Pop
            | TokenKind::Adc
            | TokenKind::Sbc
            | TokenKind::Popx
            | TokenKind::Jmp
            | TokenKind::Jsr
            | TokenKind::Rts
            | TokenKind::Int
            | TokenKind::Cli
            | TokenKind::Sti
            | TokenKind::Exit => Expr {
                kind: ExprKind::Op(op_token.0.to_owned()),
                exprs: vec![],
                line: op_token.2,
            },
            _ => return Ok(None),
        };

        self.next();

        match self.register_or_expression()? {
            Some(val) => op.exprs.push(val),
            None => return Ok(Some(op)),
        }

        // Check for an arrow
        match self.peek() {
            Some(Token(TokenKind::Arrow, _, _)) => (),
            _ => return Ok(Some(op)),
        };

        self.next();

        match self.register_or_expression() {
            Ok(Some(val)) => op.exprs.push(val),
            _ => return Err("No 2nd parameter after '->'".to_owned()),
        }

        Ok(Some(op))
    }

    fn register_or_expression(&mut self) -> Result<Option<Expr>, String> {
        let (value_token_kind, line) = match self.peek() {
            Some(t) => (t.0.to_owned(), t.2),
            None => return Ok(None),
        };

        // Check if there's a register token
        match value_token_kind {
            TokenKind::G0
            | TokenKind::G1
            | TokenKind::G2
            | TokenKind::G3
            | TokenKind::G4
            | TokenKind::G5
            | TokenKind::Ix
            | TokenKind::Pc => {
                self.next();
                return Ok(Some(Expr {
                    kind: ExprKind::Register(value_token_kind),
                    exprs: vec![],
                    line,
                }));
            }
            _ => (),
        };

        let expr = self.expression();
        match expr {
            Ok(Some(expr)) => Ok(Some(expr)),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn expression(&mut self) -> Result<Option<Expr>, String> {
        let mut expr = Expr {
            kind: ExprKind::Expression,
            exprs: vec![],
            line: 0,
        };

        let expr_token = match self.peek() {
            Some(t) => {
                expr.line = t.2;
                t
            }
            None => return Ok(None),
        };

        match self.tokens.get(self.index + 1) {
            Some(x) if matches!(x.0, TokenKind::Colon) => return Ok(None),
            _ => (),
        };

        match &expr_token.0 {
            TokenKind::Integer(v) => {
                expr.exprs.push(Expr {
                    kind: ExprKind::Integer(*v),
                    exprs: vec![],
                    line: expr_token.2,
                });

                self.next();
            }
            TokenKind::Label(n) => {
                expr.exprs.push(Expr {
                    kind: ExprKind::Label(n.to_owned()),
                    exprs: vec![],
                    line: expr_token.2,
                });
                self.next();
            }
            TokenKind::Minus | TokenKind::Plus => {
                expr.exprs.push(match self.unary()? {
                    Some(expr) => expr,
                    None => panic!("No value after unary operator."),
                });
            }
            _ => return Ok(None),
        };

        Ok(Some(expr))
    }

    fn unary(&mut self) -> Result<Option<Expr>, String> {
        let (operator_token_kind, line) = match self.peek() {
            Some(t) => (&t.0, t.2),
            None => return Ok(None),
        };

        let kind = match operator_token_kind {
            TokenKind::Plus => ExprKind::Unary(TokenKind::Plus),
            TokenKind::Minus => ExprKind::Unary(TokenKind::Minus),
            _ => return Ok(None),
        };

        self.next();

        let value = match self.peek() {
            Some(Token(TokenKind::Integer(v), _, _)) => *v,
            Some(_) => return Err("No value found after unary operator".to_owned()),
            None => return Ok(None),
        };

        self.next();

        Ok(Some(Expr {
            kind,
            exprs: vec![Expr {
                kind: ExprKind::Integer(value),
                exprs: vec![],
                line,
            }],
            line,
        }))
    }

    /*
    fn binary(&mut self) -> Result<Option<Expr>, String> {
        Err("UNIMPLEMENTED/TODO".to_owned())
    }

    fn grouping(&mut self) -> Result<Option<Expr>, String> {
        Err("UNIMPLEMENTED/TODO".to_owned())
    }
    */

    fn directive(&mut self) -> Result<Option<Expr>, String> {
        let directive_token = match self.peek() {
            Some(t) => t,
            None => return Ok(None),
        };

        let kind = match &directive_token.0 {
            TokenKind::Org
            | TokenKind::Db
            | TokenKind::Fill
            | TokenKind::Strz
            | TokenKind::FillTo => &directive_token.0,
            _ => return Ok(None),
        };

        let mut directive = Expr {
            kind: ExprKind::Directive(kind.to_owned()),
            exprs: vec![],
            line: directive_token.2,
        };

        self.next();

        loop {
            if let Some(expr) = self.expression()? {
                directive.exprs.push(expr);
                continue;
            };

            match self.peek() {
                Some(Token(TokenKind::Byte(n), _, line)) => {
                    directive.exprs.push(Expr {
                        kind: ExprKind::Byte(*n),
                        exprs: vec![],
                        line: *line,
                    });
                    self.next();
                }
                Some(Token(TokenKind::String(s), _, line)) => {
                    directive.exprs.push(Expr {
                        kind: ExprKind::String(s.to_owned()),
                        exprs: vec![],
                        line: *line,
                    });
                    self.next();
                }
                _ => break,
            };
        }

        Ok(Some(directive))
    }

    fn label(&mut self) -> Result<Option<Expr>, String> {
        let (label_token_kind, line) = match self.peek() {
            Some(t) => (&t.0, t.2),
            None => return Ok(None),
        };

        let kind = match label_token_kind {
            TokenKind::Label(n) => ExprKind::Label(n.to_owned()),
            _ => return Ok(None),
        };

        self.next();

        let colon_token = match self.peek() {
            Some(t) => t.to_owned(),
            None => return Ok(None),
        };

        match &colon_token.0 {
            TokenKind::Colon => (),
            _ => return Err("No ':' after label".to_owned()),
        };

        self.next();

        Ok(Some(Expr {
            kind,
            exprs: vec![],
            line,
        }))
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.kind)?;
        for expr in &self.exprs {
            write!(f, " {}", expr)?;
        }
        fmt::Result::Ok(())
    }
}

impl fmt::Debug for Expr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.kind)?;
        for expr in &self.exprs {
            write!(f, " {}", expr)?;
        }
        fmt::Result::Ok(())
    }
}
