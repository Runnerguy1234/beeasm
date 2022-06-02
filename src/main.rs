use std::fs;

mod codegen;
mod lexer;
mod parser;

use codegen::*;
use lexer::*;
use parser::*;

fn main() {
    let filename = "test.beeasm";
    let my_program = fs::read_to_string(filename).unwrap();

    let tokens = match Lexer::new(&my_program).tokenize() {
        Ok(vec) => {
            //println!("{:#?}", vec);
            vec
        }
        Err(e) => {
            println!("{}", e);
            return;
        }
    };

    let statements = match Parser::new(tokens).parse() {
        Ok(vec) => {
            //println!("{:#?}", vec);
            vec
        }
        Err(e) => {
            println!("{}", e);
            return;
        }
    };

    let mut parser = CodeGen::new(statements);

    loop {
        match parser.assemble_single_expr() {
            Ok(()) => (),
            Err(e) => {
                println!("{}", e);
                break;
            }
        }
    }
    for i in parser.out {
        println!("{:0>4x}", i)
    };
}
