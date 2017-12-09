extern crate canary;

use std::path::Path;

use canary::Result;

fn main() {
    if let Some(filename) = std::env::args().nth(1) {
        load(filename.as_ref())
    } else {
        repl()
    }.unwrap_or_else(|err| {
        println!("ERROR: {}", err);
    });
}

fn load(path: &Path) -> Result<()> {
    let mut world = canary::compile(path)?;

    world.exec("main", &[])?;

    Ok(())
}

fn repl() -> Result<()> {
    loop {
        use std::io::{self, BufRead, Write};

        print!(">>> ");

        io::stdout().flush()?;

        let input = {
            let mut buf = String::new();
            let stdin = io::stdin();
            let mut stdin = stdin.lock();
            stdin.read_line(&mut buf)?;
            buf
        };

        //let input = input.trim();

        if input.is_empty() {
            return Ok(());
        }

        println!("Read: {:?}", &input);

        use canary::token::Tokenizer;
        use canary::ast::parse_statements;

        let tokens = Tokenizer::new(&input).collect::<Result<Vec<_>, _>>()?;

        println!("Tokenized: {:?}", &tokens);

        let ast = parse_statements(Tokenizer::new(&input).spanned())?;

        println!("Parsed: {:?}", ast);
    }
}

