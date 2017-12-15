extern crate canary;

macro_rules! generate {
    ( $name:ident ) => {
        #[test]
        fn $name() {
            let path = format!("tests/{}.cy", stringify!($name));
            canary::compile(path)
                .and_then(|env| env.start())
                .unwrap_or_else(|err| {
                    println!("Error: {}", err);
                    panic!("Test aborted");
                });
        }
    }
}

generate!(while_loops);
generate!(arrays);
generate!(records);
generate!(functions);
generate!(patterns);
generate!(variables);
generate!(truthiness);
generate!(scopes);
generate!(strings);
